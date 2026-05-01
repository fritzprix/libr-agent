## 2024-05-24 - [Avoid over-formatting list handlers with redundant markdown blocks]

**Learning:** Returning nested Markdown tables directly to LLM without explicitly declaring backticks for data constraints (such as `agent.id` and `sessionId`) reduces parsing stability and drops vital constraints.
**Action:** When refactoring MCP Tools for tabular data, always explicitly quote ID fields and maintain strict structure without repeating conditional checks (`if !results.is_empty()`) for block-level additions.
## 2024-05-24 - [Format tabular data robustly]

**Learning:** When formatting tabular data as nested Markdown tables in MCP tool responses, raw unquoted ID fields might be lost or misinterpreted by LLMs parsing the output.
**Action:** Always explicitly quote ID fields (e.g., using backticks like `[ID]`) to maintain parsing stability for the LLM and preserve vital data constraints. Avoid repeating conditional checks for block-level additions.
