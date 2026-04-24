## 2024-05-24 - [Avoid over-formatting list handlers with redundant markdown blocks]

**Learning:** Returning nested Markdown tables directly to LLM without explicitly declaring backticks for data constraints (such as `agent.id` and `sessionId`) reduces parsing stability and drops vital constraints.
**Action:** When refactoring MCP Tools for tabular data, always explicitly quote ID fields and maintain strict structure without repeating conditional checks (`if !results.is_empty()`) for block-level additions.
## 2024-05-24 - [Pagination context loss and markdown formatting]
**Learning:** Returning unpaginated lists of complex entities without limits can easily bloat the LLM's context window. Additionally, when mapping lists to Markdown tables, ID fields must be explicitly wrapped in backticks to guarantee robust extraction by the LLM in subsequent chains.
**Action:** Always add explicit `limit` and `offset` schema parameters to MCP list tools and use `.skip(offset).take(limit)` on iterators. Output explicit IDs wrapped in backticks when returning dense markdown tables.
