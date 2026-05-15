## 2024-05-24 - Initial Setup
**Learning:** Initializing distill journal to document any critical learnings about LLM parsing and pagination.
**Action:** Use this file only for highly specific, critical insights that change our approach.
## 2024-05-24 - Prevent Pagination Bypass
**Learning:** Returning a full unpaginated entity (e.g., `rawMessage`) alongside its chunked or paginated representation defeats pagination parameters (such as `offsetChars`/`maxChars` for `readMessage` or `page`/`pageSize` for `readSession`/`search`), causing massive context window bloat.
**Action:** When implementing chunked read or pagination tools, ensure the JSON response strictly mirrors the distilled DTO and never includes the raw database record.
