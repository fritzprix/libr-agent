# Atlas Journal - Platform Log

## 2025-05-20 - [Workspace] **Platform Bug:** [Hardcoded Shell Names] **Resolved:** [Normalized Shell Tools]
- **Issue:** The `runShell` tool was only available on Unix, and `runPowerShell` was only available on Windows. This forced agents to know the underlying OS.
- **Resolution:** Normalized the tool names to `runShell` and `runInPersistentShell` across all platforms.
    - Windows: `runShell` now executes PowerShell. `runInPersistentShell` executes persistent PowerShell.
    - Unix: `runShell` executes bash/sh. `runInPersistentShell` executes persistent bash.
- **Benefit:** Agents can now use `runShell` universally without checking `std::env::consts::OS`.

## 2025-05-20 - [Browser] **Platform Bug:** [Hardcoded Path Separator] **Resolved:** [PathBuf::join]
- **Issue:** `src-tauri/src/mcp/builtin/browser/content.rs` used `format!("extracted-content/{}", file_name)`, which relies on `/` as a separator.
- **Resolution:** Replaced with `std::path::Path::new("extracted-content").join(file_name)` to use the OS-native separator.
