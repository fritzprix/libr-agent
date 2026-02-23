# 🧭 Atlas - Platform Log

This log tracks platform-specific fixes, assumptions resolved, and cross-platform abstractions implemented.

## 2025-05-23 - [src-tauri/src/mcp/builtin/workspace/persistent_shell.rs] **Platform Bug:** Hardcoded Unix path string formatting `format!("{}/.local/bin", home)` **Resolved:** Replaced with `PathBuf::join` for robust path construction.

## 2025-05-23 - [src-tauri/src/commands/workspace_commands.rs] **Platform Bug:** Missing terminal emulator support on Linux **Resolved:** Implemented fallback logic for `x-terminal-emulator`, `gnome-terminal`, `konsole`, `xfce4-terminal`, and `xterm`.

## 2025-05-23 - [src-tauri/src/commands/workspace_commands.rs] **Platform Bug:** Manual string concatenation for paths `format!("{}/{}", path, name)` **Resolved:** Replaced with `PathBuf::join` and explicit normalization for frontend consistency.

## 2025-05-23 - [src-tauri/src/commands/workspace_commands.rs] **Security Fix:** `xterm` fallback was vulnerable to command injection. **Resolved:** Updated to use `sh -c` with proper argument handling.

## 2025-05-23 - [src-tauri/src/mcp/builtin/workspace/persistent_shell.rs] **Robustness:** Manual `PATH` string concatenation. **Resolved:** Updated to use `std::env::join_paths` and `std::env::split_paths` for OS-correct separator handling.
