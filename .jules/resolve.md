## 2026-03-04 - [src-tauri/src/commands/mcp_commands.rs] **Debt Cleared:** `call_tool_unified` and associated frontend bindings were unused and deprecated. **Solution:** Removed the deprecated stack from backend and frontend, and fixed duplicate command registrations in `src-tauri/src/lib.rs`.

## 2026-03-04 - [src-tauri/src/commands/session_commands.rs] **Debt Cleared:** `switch_session` command was deprecated and unused. **Solution:** Removed the command, the `SessionSwitchRequest` struct, and its registration in `lib.rs`.

## 2026-03-04 - [src-tauri/src/agent/types.rs] **Debt Cleared:** `AgentMessageDto` type alias was deprecated. **Solution:** Replaced all usages with `Message` and removed the type alias.

## 2026-03-04 - [src-tauri/src/session_isolation/types.rs] **Debt Cleared:** Consolidated duplicate `ShellType` definitions and removed dead code markers by implementing logic to respect `shell_type` configuration in isolated processes. **Solution:** Centralized `ShellType` in `types.rs`, implemented `command()` method, updated `windows.rs` and `unix.rs` to use it, and removed misleading `get_shell_command` helper.
