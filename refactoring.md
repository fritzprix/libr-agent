
# Refactoring Plan

## Task 1 – Redesign MessageBubble.tsx

### Problem
- The current MessageBubble component handles all message types within a single component, making it overly complex.
- There are several distinct message types: pure Content, Tool Call, Tool Output (Result), and File Attachment. These should be separated for clarity and maintainability.
- If only Content is present, it should be displayed as is. However, when Tool Call, Tool Output, or File Attachment types are present, displaying Content together can lead to duplication or unnecessary complexity.

### Solution
- Implement separate Bubble components for each message type:
  - ContentBubble
  - ToolCallBubble
  - ToolOutputBubble
  - AttachmentBubble
- Create a MessageBubbleRouter component that identifies the type of each message and returns the appropriate Bubble component for rendering.

This approach will simplify the codebase, improve maintainability, and ensure that each message type is displayed clearly and without redundancy.


### MCP Response Schema

- Tool Result will be compliant MCP response schema
- Tool Result can be renderred more prettified form instead of just single liner json string

```json
{
  "jsonrpc": "2.0",
  "id": number | string,
  "result": object,
  "error": {
    "code": number,
    "message": string,
    "data": unknown
  }
}
```

## Task 2 - add Markdown Support for ContentBubble.tsx

- we have not to re-inventwheel.
- refactor the code to use `react-markdown` instead of naive implementation
