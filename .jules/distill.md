## 2024-05-24 - [Avoid over-formatting list handlers with redundant markdown blocks]

**Learning:** Returning nested Markdown tables directly to LLM without explicitly declaring backticks for data constraints (such as `agent.id` and `sessionId`) reduces parsing stability and drops vital constraints.
**Action:** When refactoring MCP Tools for tabular data, always explicitly quote ID fields and maintain strict structure without repeating conditional checks (`if !results.is_empty()`) for block-level additions.
## 2024-05-25 - [Prevent OOM in limit/offset pagination and correct has_more calculation]

**Learning:** When fetching `limit + offset + 1` records to determine if more items exist, leaving `offset` unbounded risks OOM crashes if an LLM passes an enormous value. Additionally, `has_more` must be calculated by comparing the total fetched count against `offset + limit` rather than comparing `offset + paginated_results.len()`, which creates an infinite loop trap.
**Action:** Always add a hard cap to the parsed `offset` (e.g., `.min(10_000)`) in MCP tool queries. Calculate `has_more` safely using `all_results.len() as u64 > offset.saturating_add(limit)` before splitting the slice.
