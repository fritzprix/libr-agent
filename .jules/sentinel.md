## 2025-06-13 - [Fix XSS vulnerability in resource link]
**Vulnerability:** XSS vulnerability where user-provided URIs in `resource_link` items were rendered directly into the `href` attribute of an anchor tag without validation.
**Learning:** `resource_link` components didn't use `isSafeExternalUrl` that markdown renderer used.
**Prevention:** Always validate external inputs that render into `href` to prevent Javascript protocol attacks. Ensure safe fallback (like plain `<span>`) is present when URL is unsafe.
