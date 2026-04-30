## 2024-05-24 - [Avoid over-formatting list handlers with redundant markdown blocks]

**Learning:** Returning nested Markdown tables directly to LLM without explicitly declaring backticks for data constraints (such as `agent.id` and `sessionId`) reduces parsing stability and drops vital constraints.
**Action:** When refactoring MCP Tools for tabular data, always explicitly quote ID fields and maintain strict structure without repeating conditional checks (`if !results.is_empty()`) for block-level additions.

## 2024-05-31 - [LLM Table Formatting with SuccessHints]

**Learning:** When using `SuccessHint::new()` alongside markdown table responses in list tools, the LLM parses the output much better if there's a double newline `\n\n` before the table header to separate it from preamble text like 'Scratchpad Notes (Page 1/1)'.
**Action:** Always include `\n\n` immediately before Markdown table headers in string-formatted output blocks returned to the LLM.
