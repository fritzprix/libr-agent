# 🧭 Atlas - Platform Log

This log tracks platform-specific fixes, assumptions resolved, and cross-platform abstractions implemented.

## 2024-05-18 - [Path Resolution] **Platform Bug:** [Hardcoded backslashes and panicking unwrap calls on file paths] **Resolved:** [Replaced hardcoded `['/', '\\']` with `std::path::is_separator` and added `unwrap_or_default` to `file_stem()` calls in database backup to prevent panics]

## 2025-05-23 - [src-tauri/src/mcp/builtin/workspace/persistent_shell.rs] **Platform Bug:** Hardcoded Unix path string formatting `format!("{}/.local/bin", home)` **Resolved:** Replaced with `PathBuf::join` for robust path construction.

## 2025-05-23 - [src-tauri/src/utils/terminal.rs] **Platform Bug:** Missing terminal emulator support on Linux **Status:** Linux is not yet supported — `open_in_terminal` returns a descriptive error on Linux. Implementation of fallback logic for `x-terminal-emulator`, `gnome-terminal`, etc. is deferred.

## 2025-05-23 - [src-tauri/src/services/workspace_service.rs] **Platform Bug:** Manual string concatenation for paths `format!("{}/{}", path, name)` **Resolved:** Replaced with `PathBuf::join` for cross-platform correctness.

## 2025-05-23 - [src-tauri/src/mcp/builtin/workspace/persistent_shell.rs] **Robustness:** Manual `PATH` string concatenation. **Resolved:** Updated to use `std::env::join_paths` and `std::env::split_paths` for OS-correct separator handling.

## 2025-05-24 - [src-tauri/src/utils/terminal.rs] **Platform Bug:** Missing terminal emulator support on Linux **Resolved:** Implemented prioritized fallback strategy for `gnome-terminal`, `konsole`, `xfce4-terminal`, `x-terminal-emulator`, and `xterm`.

## 2025-05-24 - [src-tauri/src/utils/terminal.rs] **Platform Bug:** Windows paths in `cmd` arguments using forward slashes **Resolved:** Normalized paths to force backslashes for `cmd`.

## 2025-05-24 - [src-tauri/src/mcp/builtin/workspace/persistent_shell.rs] **Robustness:** Unchecked dependency on `bash` **Resolved:** Added explicit existence check (`command -v bash`) before spawning.

## 2025-05-24 - [src-tauri/src/mcp/builtin/browser/content.rs] **Platform Bug:** Hardcoded forward slashes in path construction (`extracted-content/`). **Resolved:** Replaced string formatting with `PathBuf::join` to respect OS-specific path separators.

## 2026-02-24 - [src-tauri/src/mcp/builtin/workspace/export_operations.rs] **Platform Bug:** Hardcoded forward slashes in path construction. **Resolved:** Replaced string formatting with `PathBuf::join` to respect OS-specific path separators.

## 2026-02-24 - [src-tauri/src/mcp/builtin/workspace/ui_resources.rs] **Platform Bug:** Windows file paths with backslashes caused syntax errors when injected into JavaScript. **Resolved:** Implemented `serde_json::to_string` serialization for safe path injection.

## 2026-02-24 - [src-tauri/src/utils/fs.rs] **Platform Bug:** Linux file manager detection ignored user preference. **Resolved:** Reordered priority to try `xdg-open` first, respecting the desktop environment's default application.

## 2026-02-26 - [src-tauri/src/mcp/builtin/workspace/file_operations/search_query.rs] **Platform Bug:** Glob matching failed on Windows because path strings contained backslashes. **Resolved:** Implemented `matches_glob` helper to normalize paths before matching against glob patterns.

## 2026-03-01 - [src-tauri/src/mcp/builtin/workspace/code_execution/shell/persistent.rs] **Platform Bug:** Hardcoded `./` relative path prefix on Windows. **Resolved:** Replaced with logic using `std::path::MAIN_SEPARATOR` to ensure correct path formatting (e.g. `.\` on Windows).

## 2026-03-01 - [src-tauri/src/session/manager.rs] **Platform Bug:** Hardcoded path separators in macOS log directory path. **Resolved:** Replaced `Library/Logs/...` string with chained `.join()` calls for robust path construction.

## 2026-02-28 - [Server Handlers] **Platform Bug:** [Using hardcoded `/` and string concatenation for restricted path checks] **Resolved:** [Replaced with OS-conditional logic and `PathBuf::starts_with` for exact and case-insensitive component matching on Windows/Unix]

## 2026-02-28 - [Session Directory] **Platform Bug:** [Hardcoded Unix-only `welcome.sh` bash script created on all platforms] **Resolved:** [Added `#[cfg(target_os = "windows")]` logic to create an equivalent `welcome.ps1` PowerShell script instead]

## 2026-03-01 - [src/features/agent/components/AgentWorkspacePanel.tsx] **Platform Bug:** Hardcoded forward slashes and regex replacement for path construction in React frontend. **Resolved:** Replaced with Tauri's `@tauri-apps/api/path` `join` function for cross-platform correctness.

## 2026-03-03 - [Export Operations & File Export Service] **Simplification:** [Code cleanup for ZIP archive path construction] **Resolved:** [Replaced multi-step component decomposition with `to_string_lossy().replace('\\', "/")` for standard path normalization while strictly enforcing ZIP specification separators across all OSes]

## 2026-03-10 - [Database] **Platform Bug:** [Hardcoded sqlite:// formatting strings failed to connect on Windows due to unescaped backslashes in path_lossy results] **Resolved:** [Created `format_sqlite_url` to safely convert all backslashes to forward slashes before SQLite connection generation]
