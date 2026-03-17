## 2025-05-24 - Zip Slip in Skill Import

**Vulnerability:** Zip Slip vulnerability in `skill_service.rs` allowed arbitrary file write via malicious ZIP archives during skill import. The `zip::ZipArchive::extract` method (v0.6) does not sanitize paths by default.
**Learning:** Libraries like `zip` (before recent versions or specific APIs) often default to unsafe behavior for convenience. Always verify if extraction methods sanitize paths.
**Prevention:** Use `zip::ZipFile::enclosed_name()` to validate paths before extraction. Added `extract_zip_secure` helper in `utils/fs.rs` for safe extraction.

## 2025-05-24 - Symlink Traversal in SecurityValidator

**Vulnerability:** Logic flaw in `SecurityValidator::validate_path` where `!canonical.starts_with(base) && !absolute.starts_with(base)` allowed symlinks pointing outside the workspace to bypass validation because `absolute` (constructed from base) always started with base.
**Learning:** Redundant "double checks" can inadvertently create bypasses if the logic is `AND` instead of `OR`, or if one condition is trivially true. Always verify the canonical path for existing files.
**Prevention:** Fixed the condition to strictly check `canonical_path.starts_with(base)`. Added a regression test case for symlink traversal.

## 2025-05-24 - Environment Variable Leakage in MCP Processes

**Vulnerability:** MCP server processes spawned via `stdio_manager.rs` inherited all environment variables from the host process by default. This could leak sensitive secrets (e.g. `OPENAI_API_KEY`, `AWS_SECRET_KEY`) to untrusted or compromised MCP tools.
**Learning:** `tokio::process::Command` inherits the parent environment by default. Explicitly calling `.env_clear()` is required for isolation. A test explicitly asserting _against_ `env_clear()` existed, showing a misunderstanding of security requirements.
**Prevention:** Always use `cmd.env_clear()` when spawning subprocesses intended to be isolated. Use an explicit whitelist for essential system variables (`PATH`, `HOME`, etc.).

## 2026-03-01 - Environment Variable Leakage in MCP Server Verification

**Vulnerability:** When verifying an MCP server connection in `mcp_manager/operations.rs`, `tokio::process::Command` inherited the parent environment by default, potentially leaking sensitive host secrets (like `OPENAI_API_KEY`) to untrusted tools via the `test_server_connection` function.
**Learning:** Even short-lived, verification or testing processes must follow the same isolation guarantees as the main process lifecycle. `Command` builder configuration requires explicit variable clearing in all places where a child process executes untrusted components.
**Prevention:** Always use `cmd.env_clear()` before spawning any isolated process, followed by applying an explicit whitelist for required system variables (`PATH`, `HOME`, etc.).

## 2026-03-02 - [Command Injection via String Formatting in Shell Commands]

**Vulnerability:** Command injection vulnerability identified in `get_command_path` and `command_exists` when running `sh -c` with `format!("command -v {}", cmd)`. If `cmd` includes shell metacharacters, it allows executing arbitrary commands.
**Learning:** Never use string formatting (`format!`) to build arguments for shell execution (`sh -c`).
**Prevention:** Use positional arguments when calling shell commands. E.g., `sh -c 'command -v "$1"' -- cmd`.

## 2026-03-03 - Environment Variable Leakage in Persistent Shell

**Vulnerability:** In `PersistentShell::new` (`src-tauri/src/mcp/builtin/workspace/persistent_shell.rs`), `tokio::process::Command` inherited the parent environment by default, potentially leaking sensitive host secrets (like `OPENAI_API_KEY`) to untrusted code executed within the persistent shell session.
**Learning:** `env_clear()` must be used universally for all shells and external processes spawned that might execute untrusted code or commands. Process spawning in the workspace module is just as critical as MCP server spawn points.
**Prevention:** Always use `cmd.env_clear()` before spawning any shell process or isolated process. Afterwards, securely re-apply only the whitelisted essential system variables using `crate::utils::env::get_isolated_env()`.

## 2026-03-04 - Environment Variable Leakage in Bootstrap Platform Detection

**Vulnerability:** In `src-tauri/src/mcp/builtin/bootstrap/platform.rs`, `std::process::Command` inherited the parent environment by default when detecting tool versions and command paths, potentially leaking sensitive host secrets (like `OPENAI_API_KEY`) to untrusted executables.
**Learning:** Even during bootstrap phases and diagnostic checks, external processes spawned via `Command` must not inherit the parent's environment, as any executed tool (even seemingly safe ones like `node` or `python`) can run arbitrary code or scripts.
**Prevention:** Always use `cmd.env_clear()` before spawning any diagnostic or bootstrap process, and re-apply an explicit whitelist of required system variables using `crate::utils::env::get_isolated_env()`. This pattern has been extended to the shared `command_exists` utility to ensure all tool existence checks are isolated.

## 2026-03-05 - Environment Variable Leakage in System Utility Commands

**Vulnerability:** In `src-tauri/src/mcp/builtin/workspace/handlers/terminal/stop.rs`, `src-tauri/src/mcp/builtin/workspace/mod.rs`, and `src-tauri/src/utils/fs.rs`, `std::process::Command` inherited the parent environment by default when executing system utilities like `kill`, `taskkill`, `explorer`, and `open`. This could potentially leak sensitive host secrets to these executables.
**Learning:** Even built-in OS utilities and file managers executed via `Command` must not inherit the parent's environment, as any executed tool could potentially log, forward, or otherwise mishandle sensitive environment variables. Every single `Command::new` invocation must be considered a boundary.
**Prevention:** Always use `cmd.env_clear()` before spawning any system utility process, and re-apply an explicit whitelist of required system variables using `crate::utils::env::get_isolated_env()`. This ensures a secure baseline for all external process execution.

## 2026-03-06 - Environment Variable Leakage in Terminal Launcher

**Vulnerability:** In `src-tauri/src/utils/terminal.rs`, `std::process::Command` inherited the parent environment by default when launching an external OS terminal instance (`cmd.exe`, `osascript`, `gnome-terminal`, etc.). This could potentially leak sensitive host secrets (like `OPENAI_API_KEY`) to the new terminal session where untrusted user-level code, MCP tools, or commands might execute.
**Learning:** Every `Command::new` invocation must be considered a security boundary. Even launching a standard user terminal instance from within an application must be isolated, as the child terminal session inherits the parent's environment, exposing any runtime secrets injected into the Tauri application.
**Prevention:** Always invoke `crate::utils::env::apply_isolated_env(&mut cmd)` after creating a `Command` with `Command::new` and before spawning it, to explicitly clear the environment (`env_clear()`) and re-apply only a whitelist of required system variables.
