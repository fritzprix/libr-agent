---
description: Run Rust formatting, clippy, and type checks
---

Run Rust backend quality checks.

1. `pnpm rust:fmt:check` - Verify rustfmt formatting
2. `pnpm rust:clippy:all` - Run Clippy linter
3. `pnpm rust:check:all` - Run cargo check for all targets

All checks must pass before committing Rust changes.
