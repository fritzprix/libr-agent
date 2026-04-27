## 2024-05-24 - [Avoid over-formatting list handlers with redundant markdown blocks]

**Learning:** Returning nested Markdown tables directly to LLM without explicitly declaring backticks for data constraints (such as `agent.id` and `sessionId`) reduces parsing stability and drops vital constraints.
**Action:** When refactoring MCP Tools for tabular data, always explicitly quote ID fields and maintain strict structure without repeating conditional checks (`if !results.is_empty()`) for block-level additions.

## 2024-05-25 - [Removed rawMessage bloat from read_message]

**Learning:** Dropping fields from an existing MCP tool response can introduce a regression risk if any downstream agents or prompts secretly rely on extracting IDs or metadata from the raw JSON object. Always ask for permission before dropping existing fields if the field might be relied upon.
**Action:** When asked to remove fields that look like context bloat, double check with the user or codebase references to ensure it is safe to remove, and only proceed after explicit verification.
