# Rust Backend Conventions

Guidelines for writing Rust code in `src-tauri/`.

## Naming Conventions

- **snake_case** for functions, variables, and module names
- **PascalCase** for types, structs, and enums
- Comprehensive documentation comments (`///`) for public APIs
- Handle errors explicitly using `Result<T, E>` types

## Method vs. Associated Function

### Method

Takes `self` (or `&self`, `&mut self`) as the first parameter in an `impl` block.
→ Called through instance: `self.method_name(...)`

### Associated Function

No `self` parameter.
→ Called through type name: `TypeName::function_name(...)`

```rust
impl MyStruct {
    // Method: requires self
    fn do_something(&self, arg: i32) { ... }

    // Associated function: no self
    fn helper(arg: i32) { ... }
}

let obj = MyStruct::new();
obj.do_something(42);           // ✅ Method call
MyStruct::helper(42);           // ✅ Associated function call
```

### Error Prevention Checklist

- If using `self` in a function, declare `self` as the first parameter.
- Associated functions cannot use `self`.
- Call methods through instances, associated functions through type names.

### Common Mistake

```rust
// ❌ WRONG: self used in associated function
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
    self.copy_dir_contents(&src_path, &dst_path)?;
}

// ✅ CORRECT: Call through type name
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
    SessionManager::copy_dir_contents(&src_path, &dst_path)?;
}

// ✅ CORRECT: Add self parameter to make it a method
fn copy_dir_contents(&self, src: &Path, dst: &Path) -> Result<(), String> {
    self.copy_dir_contents(&src_path, &dst_path)?;
}
```

> **Tip:** The Rust compiler clearly indicates these mistakes — read error messages carefully. Use "Go to Definition" in IDEs to check if a function is a method or associated function.

## Rust Test Architecture — CRITICAL

CI / `pnpm rust:test` runs **integration tests under `src-tauri/tests/`**, NOT `cargo test --lib`. This means:

- `#[cfg(test)]` blocks inside `src/` **are never executed in CI**
- Only tests in the `tests/` directory (integration tests) run in CI
- On Windows, `cargo test --lib` also crashes with `STATUS_ENTRYPOINT_NOT_FOUND`

**Rule: All Rust tests MUST be written as integration tests in `src-tauri/tests/`.**

### Resource limits (32GB machines still OOM)

Each `tests/*.rs` file is a **separate Cargo test binary** (~600MB+ with Tauri).  
`cargo test --tests` links many of them in one build graph and will thrash or freeze a 32GB system.

- **Full suite:** `pnpm rust:test` → `scripts/run-rust-tests-sequential.cjs` (one `--test` target at a time, `CARGO_BUILD_JOBS=1`).
- **Iterate:** `pnpm rust:test --test <name>`.
- **Never** run bare `cargo test` / `cargo test --tests` locally.
- `scripts/run-rust-command.cjs test` **without** `--test <name>` auto-delegates to the sequential runner (refuses multi-target link).
- Prefer adding modules under `tests/integration/` (single `integration_tests` binary on Linux/macOS) instead of new root-level `tests/foo.rs` crates.
- Root-level crates remain for Windows-safe coverage; `integration_tests` is `#![cfg(not(windows))]`.
- Optional: `pnpm rust:test:consolidate` merges root crates into `integration/` (merge-safe; does not replace Windows strategy).

```
src-tauri/
└── tests/
    ├── integration_tests.rs   ← consolidated suite (Linux/macOS)
    ├── integration/           ← modules for that suite
    └── text_encoding_tests.rs ← example root-level Windows-safe target
```

### Integration Test Boilerplate

```rust
use sqlx::sqlite::SqlitePoolOptions;
use sea_orm::SqlxSqliteConnector;
use tauri_mcp_agent_lib::migration::Migrator;
use sea_orm_migration::MigratorTrait;

async fn setup_db() -> sea_orm::DatabaseConnection {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);
    Migrator::up(&db, None).await.unwrap();
    db
}
```

Do **not** add `#[cfg(test)]` blocks to lib source files — they give a false sense of test coverage.

## Design Principles

When refactoring or implementing new features:

1. **DRY** — Eliminate duplication through abstraction and shared utilities
2. **SRP** — Each module/function has one clear purpose
3. **OCP** — Code open for extension, closed for modification
4. **ISP** — Keep interfaces simple and focused
5. **DIP** — Depend on traits/abstractions, not concrete implementations

Extract common patterns into `src-tauri/src/utils/`. Document design decisions in `docs/refactoring/`.
