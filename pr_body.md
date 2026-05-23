### 💡 What

- Added `limit` and `offset` schema parameters to the `listTools` MCP tool in `mcp_manager`.
- Mapped the raw JSON list response to a dense, paginated Markdown table.
- Sanitized descriptions by escaping pipe characters and replacing newlines.

### 🎯 Why

- Resolves context window bloat when returning hundreds of cached tools from builtin or external servers.
- Improves LLM readability by presenting tool properties (Source, Server, Tool, Status, Description) in an aligned table layout instead of a sprawling hierarchical list.

### 📉 Token Impact

- Output token usage scales linearly with `limit` (default: 50 rows) instead of growing uncontrollably with the user's inventory size.
- Dense table formatting eliminates redundant structural prefixes and whitespace from previous list representations, compressing the payload per tool row.

### 🛠️ Error Recovery

- Includes actionable pagination hints at the bottom of the table, explicitly directing the LLM to call the tool again with `offset` to fetch the next block of results if more tools exist.
