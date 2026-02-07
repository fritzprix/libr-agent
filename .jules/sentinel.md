## 2025-05-21 - PowerShell Wrapper Injection

**Vulnerability:** Command injection in PowerShell error handling wrapper due to unsafe string interpolation allowed attackers to escape the try/catch block.
**Learning:** String interpolation of code into other code (metaprogramming) is inherently unsafe without strict escaping or encoding.
**Prevention:** Use Base64 encoding to encapsulate dynamic code blocks when passing them to interpreters like PowerShell, preventing syntax manipulation.
