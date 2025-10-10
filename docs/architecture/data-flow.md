# Data Flow in SynapticFlow

This document illustrates the flow of data between the React frontend, the Tauri backend, and the MCP servers.

## 1. User Sends a Message

1. **`Chat.tsx`**: The user types a message and clicks "Send".
2. **`use-chat.tsx` Hook**: The `useChat` hook is called, which adds the user's message to the session history.
3. **AI Service**: The message is sent to the configured AI service (e.g., OpenAI, Anthropic).

## 2. AI Assistant Responds with a Tool Call

1. **AI Service**: The AI service processes the prompt and decides to use a tool. It returns a response containing a `tool_calls` array.
2. **`use-chat.tsx` Hook**: The hook receives the response and identifies the tool call.
3. **`ToolCaller.tsx`**: The `ToolCaller` orchestrator is invoked to handle the tool call.

## 3. Frontend to Backend Communication

1. **`ToolCaller.tsx`**: The orchestrator determines which MCP server hosts the required tool.
2. **`tauri-mcp-client.ts`**: The client calls the appropriate Tauri command, such as `call_mcp_tool`.
3. **`invoke`**: The `@tauri-apps/api/tauri` `invoke` function sends the request from the JavaScript context to the Rust backend.

## 4. Backend to MCP Server Communication

1. **`lib.rs`**: The `call_mcp_tool` command in Rust is executed.
2. **`MCPServerManager`**: The manager finds the correct `MCPConnection` for the target server.
3. **`rmcp`**: The `rmcp` library sends the tool call request to the external MCP server process over the configured transport (e.g., stdio).

## 5. MCP Server Executes the Tool

1. **MCP Server**: The external process receives the request, parses it, and executes the corresponding tool logic.
2. **MCP Server**: The server sends the result back to the SynapticFlow backend as a JSON-RPC response.

## 6. Data Returns to the Frontend

1. **`lib.rs`**: The Rust backend receives the result from the MCP server.
2. **`invoke`**: The result is returned to the JavaScript promise that `invoke` created.
3. **`ToolCaller.tsx`**: The `ToolCaller` receives the result and formats it as a `tool` message.
4. **`use-chat.tsx` Hook**: The tool result message is added to the session history, and the conversation is sent back to the AI service to generate a final response.
5. **`Chat.tsx`**: The UI updates to display the tool call, the tool output, and the final assistant message.
