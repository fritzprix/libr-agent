## 2025-05-21 - PowerShell Wrapper Injection

**Vulnerability:** Command injection in PowerShell error handling wrapper due to unsafe string interpolation allowed attackers to escape the try/catch block.
**Learning:** String interpolation of code into other code (metaprogramming) is inherently unsafe without strict escaping or encoding.
**Prevention:** Use Base64 encoding to encapsulate dynamic code blocks when passing them to interpreters like PowerShell, preventing syntax manipulation.

## 2025-05-22 - macOS Command Injection Mitigation

**Vulnerability:** Manual escaping of user-provided paths in AppleScript for terminal launching was complex and potentially vulnerable to injection.
**Learning:** Avoid string interpolation and manual escaping when system APIs or commands treat arguments as data (e.g., `open -a`).
**Prevention:** Replaced `osascript` script construction with direct execution of `open -a Terminal <path>`, leveraging the OS's argument handling to prevent injection.
