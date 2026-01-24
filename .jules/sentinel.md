# Sentinel Security Audit Log

## 2025-05-18 - [AgentMessageRenderer] **Threat:** Cross-Site Scripting (XSS) **Mitigation:** Enforced HTML escaping and sandboxed UI resources
**Vulnerability:** The `AgentMessageRenderer` component was using `skipHtml={false}` which explicitly enabled HTML rendering in older versions of `react-markdown` (and potentially caused confusion or unsafe behavior if configuration changed). Additionally, `UIResourceRenderer` was rendering external content without strict isolation.
**Mitigation:**
1. Removed `skipHtml={false}` to rely on `react-markdown`'s default safe behavior (HTML escaping).
2. Added `sandbox="allow-scripts allow-popups allow-forms"` to `UIResourceRenderer`'s `iframeProps` to strictly confine the execution environment of rendered UI resources, preventing top-level navigation, cookie access, and other potential exploits.
**Severity:** Medium (Potential XSS if malicious content is injected via LLM or tool outputs).

## 2025-05-18 - [ContentStore] **Threat:** Arbitrary File Read (Potential) **Mitigation:** Risk Identified
**Vulnerability:** The `add_content` tool allows reading files from any path provided via `fileUrl` (e.g., `file:///etc/passwd`).
**Risk:** While this may be intended for a local agent with user permissions, it poses a risk if the agent is prompted to exfiltrate sensitive files.
**Recommendation:** Implement a "Workspace" confinement policy that restricts file access to specific allowed directories (e.g., project root, user-defined workspace).
**Severity:** High (but potentially Feature-as-Designed).
