## 2024-05-24 - [Avoid over-formatting list handlers with redundant markdown blocks]

**Learning:** Returning nested Markdown tables directly to LLM without explicitly declaring backticks for data constraints (such as `agent.id` and `sessionId`) reduces parsing stability and drops vital constraints.
**Action:** When refactoring MCP Tools for tabular data, always explicitly quote ID fields and maintain strict structure without repeating conditional checks (`if !results.is_empty()`) for block-level additions.
## 2024-05-24 - [Format Explore Context output as dense Markdown tables]
**Learning:** Returning large, verbose lists (e.g., node graphs and linked chunks) without structured Markdown tables inflates token count and reduces LLM readability. Also, failing to quote IDs (e.g. `` `123` ``) inside tables can reduce the LLM's ability to chain them reliably.
**Action:** When extracting nested entities and hierarchical data, immediately map the unrolled context arrays into Markdown tables, explicitly quoting ID fields to preserve token limits and semantic stability.
