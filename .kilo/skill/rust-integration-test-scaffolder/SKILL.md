---
name: rust-integration-test-scaffolder
description: Scaffold integration tests in src-tauri/tests/ following the project's strict rules. Use when creating new Rust tests, ensuring CI compatibility, or preventing false sense of test coverage from unit tests in src/.
---

# Rust Integration Test Scaffolder

Scaffold integration tests for LibrAgent following the project's strict testing rules.

## Critical Rule

**CI / `pnpm rust:test` runs `--lib` first, then each `tests/*.rs` target one at a time.**

- `#[cfg(test)]` blocks inside `src/` **do run** via the sequential `--lib` step
- Prefer `tests/` for full Tauri/DB harness coverage (especially Windows-safe root crates)
- Windows `--lib` needs the ComCtl32 v6 linker dependency from `build.rs` (#2122)
- `test = false` in `Cargo.toml` keeps bare `cargo test` from auto-building the lib harness; the runner passes `--lib` explicitly

## Template

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

#[tokio::test]
async fn test_descriptive_name() {
    let db = setup_db().await;
    // Test implementation here
}
```

## Test Categories

### Database/Repository Tests

- Test SeaORM entity mappings
- Test repository queries
- Test migration up/down
- Test foreign key constraints

### MCP Integration Tests

- Test builtin server tool execution
- Test session isolation
- Test `MCPServiceProxy` routing
- Test external MCP server connection

### Command Handler Tests

- Test Tauri command inputs/outputs
- Test error handling paths
- Test permission validation

## Placement

- `src-tauri/tests/<module>_tests.rs` - Integration tests by module
- Use descriptive filenames matching the module being tested

## Verification

```bash
# Run all integration tests (CI command)
cargo test --tests

# Run specific test file
cargo test --test <file_stem>
```
