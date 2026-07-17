---
description: Run all tests (frontend + Rust integration)
---

Run the complete test suite for LibrAgent.

1. Frontend tests: `pnpm test:run:validate` (Vitest)
2. Rust integration tests: `pnpm rust:test` (cargo test --tests)

Note: CI runs `cargo test --tests`, not `cargo test --lib`. Unit tests inside `src/` are disabled by `test = false` in Cargo.toml. All Rust tests must be integration tests in `src-tauri/tests/`.
