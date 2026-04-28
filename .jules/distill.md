## 2024-05-24 - [Avoid over-formatting list handlers with redundant markdown blocks]

**Learning:** Returning nested Markdown tables directly to LLM without explicitly declaring backticks for data constraints (such as `agent.id` and `sessionId`) reduces parsing stability and drops vital constraints.
**Action:** When refactoring MCP Tools for tabular data, always explicitly quote ID fields and maintain strict structure without repeating conditional checks (`if !results.is_empty()`) for block-level additions.

## 2024-05-25 - [Sanitize arbitrary text when generating Markdown tables]
**Learning:** When directly converting arbitrary content (like user notes or logs) into Markdown tables, newline characters (`\n`) and pipe characters (`|`) will prematurely break rows or cell boundaries, causing the table syntax to fail for the LLM.
**Action:** Always safely sanitize dynamic cell values by applying `.replace('|', "\\|").replace('\n', " ")` before inserting them into a generated Markdown table.
