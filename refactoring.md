# Refactoring Plan

## Task 1 - Define Schema compatible to MCP (Model Context Protocol)

- Documented response schema

  ```json
  {
    "jsonrpc": "2.0",
    "id": 2,
    "result": {
      "content": [
        {
          "type": "text",
          "text": "Current weather in New York:\nTemperature: 72°F\nConditions: Partly cloudy"
        }
      ],
      "isError": false
    }
  }
  ```

  - Text Content

    ```json
    {
      "type": "text",
      "text": "Tool result text"
    }
    ```

- the tool call result is returned from
