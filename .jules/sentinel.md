## 2025-05-21 - PowerShell Wrapper Injection

**Vulnerability:** Command injection in PowerShell error handling wrapper due to unsafe string interpolation allowed attackers to escape the try/catch block.
**Learning:** String interpolation of code into other code (metaprogramming) is inherently unsafe without strict escaping or encoding.
**Prevention:** Use Base64 encoding to encapsulate dynamic code blocks when passing them to interpreters like PowerShell, preventing syntax manipulation.

## 2025-05-23 - Unrestricted File Read Exposure

**Vulnerability:** The `read_file` Tauri command, exposed to the frontend, allowed arbitrary file reads (absolute paths) without workspace restriction. Although unused, it presented a high-risk attack surface if XSS occurred.
**Learning:** Dead code in security-sensitive areas (IPC commands) is a latent vulnerability. If a command is not used, it should not be exposed.
**Prevention:** Audit all exposed IPC commands against actual frontend usage. Deprecate and remove unused commands. Harden `read_dropped_file` to reject hidden files/directories as a heuristic for sensitive configuration.
## 2025-05-22 - Unbounded File Read DoS & TOCTOU

**Vulnerability:** `tokio::fs::read` reads entire files into memory without size checks, enabling DoS. Checking metadata size before reading introduces a TOCTOU race condition if the file grows between check and read.
**Learning:** Metadata checks are insufficient for limiting resource usage during file I/O because file state is mutable.
**Prevention:** Use `File::open` combined with `take(limit)` (or `read_to_end` with a capped buffer) to strictly enforce read limits at the I/O operation level.
