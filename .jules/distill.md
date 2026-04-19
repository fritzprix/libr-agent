## 2024-05-24 - [Avoid over-formatting list handlers with redundant markdown blocks]

**Learning:** Returning nested Markdown tables directly to LLM without explicitly declaring backticks for data constraints (such as `agent.id` and `sessionId`) reduces parsing stability and drops vital constraints.
**Action:** When refactoring MCP Tools for tabular data, always explicitly quote ID fields and maintain strict structure without repeating conditional checks (`if !results.is_empty()`) for block-level additions.
## 2024-05-27 - [Format Markdown tables with strictly explicitly quoted fields]
**Learning:** Building on the previous journal entry regarding Markdown parsing constraints, LLMs require robust escaping when string properties contain Markdown characters (`|`, `\n`) and strict quoting for selector paths so that chaining them back into browser tools does not fail.
**Action:** When converting lists into Markdown tables, explicitly sanitize arbitrary content with `.replace("|", "\\|").replace("\n", " ")` and strictly use backticks to enclose any Index or ID identifiers.
