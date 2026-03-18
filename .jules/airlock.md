## 2025-03-17 - [UI Bleed via MCP Protocol]
**Learning:** Sending fully formed HTML/CSS (even via internal `ui://` URIs meant for iframe rendering) violates strict frontend/backend separation (Airlock protocol) because it couples presentation styling (`color: white`, form structures, DOM event binding) within the Rust backend logic.
**Action:** Instead of generating raw HTML strings, return semantic JSON DTOs mapped to the URI scheme, and build corresponding native React components that consume these DTOs for rendering and action handling (e.g. `InteractiveShellInput`).
