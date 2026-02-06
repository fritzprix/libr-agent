## 2026-02-05 - src-tauri/src/mcp/builtin/workspace/code_execution/shell.rs

**Split:** `handlers.rs`, `persistent.rs`, `isolated.rs`, `async_exec.rs`
**Improvement:** Decoupled MCP request handling from core execution logic. Separated persistent shell state management from isolated process execution. Reduced file size from >1100 lines to focused <300 line modules.

## 2025-02-06 - src-tauri/src/session_isolation.rs

**Split:** `types.rs`, `common.rs`, `platforms/` (linux, macos, windows)
**Improvement:** Decomposed cross-platform isolation logic into OS-specific modules. Removed large `match` blocks and `cfg` spaghetti from the main manager. Types are now isolated from logic.
