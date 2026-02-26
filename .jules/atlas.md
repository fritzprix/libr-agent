# 🧭 Atlas - Platform Log

This log tracks platform-specific fixes, assumptions resolved, and cross-platform abstractions implemented.

## 2025-05-23 - [src-tauri/src/mcp/builtin/workspace/persistent_shell.rs] **Platform Bug:** Hardcoded Unix path string formatting `format!("{}/.local/bin", home)` **Resolved:** Replaced with `PathBuf::join` for robust path construction.

## 2025-05-23 - [src-tauri/src/utils/terminal.rs] **Platform Bug:** Missing terminal emulator support on Linux **Status:** Linux is not yet supported — `open_in_terminal` returns a descriptive error on Linux. Implementation of fallback logic for `x-terminal-emulator`, `gnome-terminal`, etc. is deferred.

## 2025-05-23 - [src-tauri/src/services/workspace_service.rs] **Platform Bug:** Manual string concatenation for paths `format!("{}/{}", path, name)` **Resolved:** Replaced with `PathBuf::join` for cross-platform correctness.

## 2025-05-23 - [src-tauri/src/mcp/builtin/workspace/persistent_shell.rs] **Robustness:** Manual `PATH` string concatenation. **Resolved:** Updated to use `std::env::join_paths` and `std::env::split_paths` for OS-correct separator handling.

## 2025-05-24 - [src-tauri/src/utils/terminal.rs] **Platform Bug:** Missing terminal emulator support on Linux **Resolved:** Implemented prioritized fallback strategy for `gnome-terminal`, `konsole`, `xfce4-terminal`, `x-terminal-emulator`, and `xterm`.

## 2025-05-24 - [src-tauri/src/utils/terminal.rs] **Platform Bug:** Windows paths in `cmd` arguments using forward slashes **Resolved:** Normalized paths to force backslashes for `cmd`.

## 2025-05-24 - [src-tauri/src/mcp/builtin/workspace/persistent_shell.rs] **Robustness:** Unchecked dependency on `bash` **Resolved:** Added explicit existence check (`command -v bash`) before spawning.

## 2026-02-24 - [src-tauri/src/mcp/builtin/workspace/export_operations.rs] **Platform Bug:** Hardcoded forward slashes in path construction. **Resolved:** Replaced string formatting with `PathBuf::join` to respect OS-specific path separators.

## 2026-02-24 - [src-tauri/src/mcp/builtin/workspace/ui_resources.rs] **Platform Bug:** Windows file paths with backslashes caused syntax errors when injected into JavaScript. **Resolved:** Implemented `serde_json::to_string` serialization for safe path injection.

## 2026-02-24 - [src-tauri/src/utils/fs.rs] **Platform Bug:** Linux file manager detection ignored user preference. **Resolved:** Reordered priority to try `xdg-open` first, respecting the desktop environment's default application.

## 2026-02-26 - [src-tauri/src/mcp/builtin/workspace/file_operations/search_query.rs] **Platform Bug:** Glob matching failed on Windows because path strings contained backslashes. **Resolved:** Implemented `matches_glob` helper to normalize paths before matching against glob patterns.

## 2026-02-26 - [src-tauri/src/mcp/builtin/workspace/code_execution/shell/persistent.rs] **Platform Bug:** Hardcoded `./` prefix for relative working directory display. **Resolved:** Used `std::path::MAIN_SEPARATOR` to support `.\` on Windows.
