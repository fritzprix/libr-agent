## 2024-05-24 - Initial Setup
**Learning:** Initializing distill journal to document any critical learnings about LLM parsing and pagination.
**Action:** Use this file only for highly specific, critical insights that change our approach.
## 2024-05-24 - Prevent Pagination Bypass
**Learning:** Returning a full unpaginated entity (e.g., `rawMessage`) alongside its chunked or paginated representation defeats pagination parameters (such as `offsetChars`/`maxChars` for `readMessage` or `page`/`pageSize` for `readSession`/`search`), causing massive context window bloat.
**Action:** When implementing chunked read or pagination tools, ensure the JSON response strictly mirrors the distilled DTO and never includes the raw database record.
## 2026-05-20 - Markdown Table Density
**Learning:** Converting grouped Markdown lists into dense Markdown tables significantly improves token density and LLM readability for tools returning collections like `listTools`.
**Action:** When mapping list responses, flatten grouped hierarchies into Markdown tables and escape pipe characters and newlines in description fields.
## 2024-06-01 - Raw List Tools Array Removal
**Learning:** Returning large, raw JSON arrays of tool list configurations alongside a dense Markdown table significantly bloats the context window and defeats the purpose of chunked pagination, causing prompt overflow on instances with many registered MCP servers and builtin tools.
**Action:** When an MCP endpoint (like tool list) creates an LLM-friendly Markdown table, actively prune the underlying unpaginated or raw structured object collections from the hidden JSON response data.
