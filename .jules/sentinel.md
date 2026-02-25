## 2025-05-24 - Zip Slip in Skill Import

**Vulnerability:** Zip Slip vulnerability in `skill_service.rs` allowed arbitrary file write via malicious ZIP archives during skill import. The `zip::ZipArchive::extract` method (v0.6) does not sanitize paths by default.
**Learning:** Libraries like `zip` (before recent versions or specific APIs) often default to unsafe behavior for convenience. Always verify if extraction methods sanitize paths.
**Prevention:** Use `zip::ZipFile::enclosed_name()` to validate paths before extraction. Added `extract_zip_secure` helper in `utils/fs.rs` for safe extraction.

## 2025-05-24 - Environment Variable Leakage in MCP Processes

**Vulnerability:** MCP server processes spawned via `stdio_manager.rs` inherited all environment variables from the host process by default. This could leak sensitive secrets (e.g. `OPENAI_API_KEY`, `AWS_SECRET_KEY`) to untrusted or compromised MCP tools.
**Learning:** `tokio::process::Command` inherits the parent environment by default. Explicitly calling `.env_clear()` is required for isolation. A test explicitly asserting *against* `env_clear()` existed, showing a misunderstanding of security requirements.
**Prevention:** Always use `cmd.env_clear()` when spawning subprocesses intended to be isolated. Use an explicit whitelist for essential system variables (`PATH`, `HOME`, etc.).
