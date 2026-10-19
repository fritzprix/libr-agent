---
description: Run all tests (frontend + Rust integration)
---

Run the complete test suite for LibrAgent.

1. Frontend tests: `pnpm test:run:validate` (Vitest)
2. Rust tests: `pnpm rust:test` (`--lib` then each `tests/*.rs` target, sequentially)

Note: `test = false` in Cargo.toml only blocks bare `cargo test` auto-selecting the lib harness; the sequential runner passes `--lib` explicitly (#2122). Prefer `src-tauri/tests/` for harness-heavy coverage.
