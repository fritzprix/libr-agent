## 2025-05-21 - PowerShell Wrapper Injection

**Vulnerability:** Command injection in PowerShell error handling wrapper due to unsafe string interpolation allowed attackers to escape the try/catch block.
**Learning:** String interpolation of code into other code (metaprogramming) is inherently unsafe without strict escaping or encoding.
**Prevention:** Use Base64 encoding to encapsulate dynamic code blocks when passing them to interpreters like PowerShell, preventing syntax manipulation.

## 2025-05-21 - Persistent Shell Isolation Bypass

**Vulnerability:** `PersistentShell` (interactive mode) bypassed the centralized `SessionIsolationManager`, allowing execution without enforced security policies (e.g., restricted PATH, env sanitization).
**Learning:** Separate execution paths for "one-shot" and "interactive/persistent" commands often lead to security drift where one path lacks the protections of the other.
**Prevention:** Unified process creation under a single `SessionIsolationManager` that handles both interactive (direct binary execution) and non-interactive (shell-wrapped) modes, ensuring consistent policy application.
