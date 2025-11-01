# Type Definitions

This document provides definitions for the core data structures used in LibrAgent, covering both the Rust backend and the TypeScript frontend.

## Rust Backend Types (`src-tauri/src/mcp.rs`)

These types are central to the MCP server management and tool communication logic.

### MCPServerConfig

**Purpose**: Defines the configuration for launching and connecting to an MCP server.

**Source**: [`src-tauri/src/mcp.rs`](../src-tauri/src/mcp.rs)

**Fields**:

- `name: String`: A unique name for the server instance.
- `command: Option<String>`: The command to execute for stdio-based servers (e.g., `npx`).
- `args: Option<Vec<String>>`: Arguments to pass to the command.
- `env: Option<HashMap<String, String>>`: Environment variables to set for the server process.
- `transport: String`: The communication protocol. Can be `"stdio"`, `"http"`, or `"websocket"`. Defaults to `"stdio"`.
- `url: Option<String>`: The URL for HTTP or WebSocket servers.
- `port: Option<u16>`: The port for HTTP or WebSocket servers.

**Example (TypeScript)**:

```typescript
const config = {
  name: 'filesystem',
  command: 'npx',
  args: ['-y', '@modelcontextprotocol/server-filesystem', '/tmp'],
  transport: 'stdio',
};
```

---

### MCPTool

**Purpose**: Represents a single tool provided by an MCP server.

**Source**: [`src-tauri/src/mcp.rs`](../src-tauri/src/mcp.rs)

**Fields**:

- `name: String`: The name of the tool.
- `title: Option<String>`: A human-readable title for the tool.
- `description: String`: A detailed description of what the tool does.
- `input_schema: JSONSchema`: The JSON schema defining the tool's input parameters.
- `output_schema: Option<JSONSchema>`: The JSON schema for the tool's output (optional).
- `annotations: Option<MCPToolAnnotations>`: Additional metadata (optional).

---

### ToolCallResult

**Purpose**: Encapsulates the result of a tool call, following the JSON-RPC 2.0 specification.

**Source**: [`src-tauri/src/mcp.rs`](../src-tauri/src/mcp.rs)

**Fields**:

- `jsonrpc: String`: Always `"2.0"`.
- `id: serde_json::Value`: The request ID.
- `result: Option<serde_json::Value>`: The successful result of the tool call.
- `error: Option<MCPError>`: An error object if the call failed.

---

### JSONSchema

**Purpose**: A detailed, serializable representation of a JSON schema, used for defining tool inputs and outputs.

**Source**: [`src-tauri/src/mcp.rs`](../src-tauri/src/mcp.rs)

**Structure**: This is a complex enum (`JSONSchemaType`) that can represent all standard JSON schema types (`string`, `number`, `object`, `array`, etc.) with their respective validation keywords.

## Frontend Types (`src/models/chat.ts`)

These types are used in the React application to manage chat state and interactions.

### Message

**Purpose**: Represents a single message in a chat session.

**Source**: [`src/models/chat.ts`](../src/models/chat.ts)

**Fields**:

- `id: string`: A unique identifier for the message.
- `sessionId: string`: The ID of the session this message belongs to.
- `role: 'user' | 'assistant' | 'system' | 'tool'`: The role of the message sender.
- `content: string`: The text content of the message.
- `tool_calls?: ToolCall[]`: Optional array of tool calls requested by the assistant.
- `tool_call_id?: string`: The ID of the tool call this message is a result of.
- `isStreaming?: boolean`: `true` if the message content is still being streamed.
- `createdAt?: Date`: The timestamp when the message was created.
