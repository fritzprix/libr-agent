## 2024-05-24 - [Avoid over-formatting list handlers with redundant markdown blocks]

**Learning:** Returning nested Markdown tables directly to LLM without explicitly declaring backticks for data constraints (such as `agent.id` and `sessionId`) reduces parsing stability and drops vital constraints.
**Action:** When refactoring MCP Tools for tabular data, always explicitly quote ID fields and maintain strict structure without repeating conditional checks (`if !results.is_empty()`) for block-level additions.
## 2026-04-25 - [Paginate sub-agent session lists]
**Learning:** Missing pagination on dynamically growing arrays like delegated session lists risks context bloat as a workflow progresses.
**Action:** Always enforce 'limit' and 'offset' pagination on any tool that returns a list of spawned items to preserve context boundaries.
