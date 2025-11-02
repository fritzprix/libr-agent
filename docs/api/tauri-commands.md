# Tauri Commands

This document provides a comprehensive reference for all Tauri commands available in LibrAgent.

## Server Management

### start_mcp_server

**Purpose**: Starts an MCP server and initializes its toolset.

**Source**: [`src-tauri/src/lib.rs:20-25`](../src-tauri/src/lib.rs#L20-L25)

**Parameters**:

- `config: MCPServerConfig` - The server configuration object. See [`types.md#mcpserverconfig`](./types.md#mcpserverconfig) for details.

**Returns**:

- `Result<String, String>` - Returns the server name on success, or an error message on failure.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/tauri';

const config = {
  name: 'filesystem',
  command: 'npx',
  args: ['-y', '@modelcontextprotocol/server-filesystem', '/tmp'],
};

try {
  const serverName = await invoke<string>('start_mcp_server', { config });
  console.log(`Server started: ${serverName}`);
} catch (error) {
  console.error('Failed to start server:', error);
}
```

**Error Cases**:

- Server executable not found.
- Port is already in use.
- Initialization failure due to invalid configuration.

**Related**:

- [`stop_mcp_server`](#stop_mcp_server)
- [`check_server_status`](#check_server_status)
- [`MCPServerConfig`](./types.md#mcpserverconfig)

---

### stop_mcp_server

**Purpose**: Stops a running MCP server.

**Source**: [`src-tauri/src/lib.rs:27-32`](../src-tauri/src/lib.rs#L27-L32)

**Parameters**:

- `server_name: String` - The name of the server to stop.

**Returns**:

- `Result<(), String>` - Returns `Ok` on success, or an error message on failure.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/tauri';

try {
  await invoke('stop_mcp_server', { serverName: 'filesystem' });
  console.log('Server stopped successfully.');
} catch (error) {
  console.error('Failed to stop server:', error);
}
```

**Related**:

- [`start_mcp_server`](#start_mcp_server)
- [`check_all_servers_status`](#check_all_servers_status)

## Tool Operations

### call_mcp_tool

**Purpose**: Calls a specific tool on a running MCP server.

**Source**: [`src-tauri/src/lib.rs:34-41`](../src-tauri/src/lib.rs#L34-L41)

**Parameters**:

- `server_name: String` - The name of the server hosting the tool.
- `tool_name: String` - The name of the tool to call.
- `arguments: serde_json::Value` - The arguments to pass to the tool, as a JSON object.

**Returns**:

- `ToolCallResult` - The result of the tool call. See [`types.md#toolcallresult`](./types.md#toolcallresult).

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/tauri';

try {
  const result = await invoke('call_mcp_tool', {
    serverName: 'filesystem',
    toolName: 'writeFile',
    arguments: { path: '/tmp/test.txt', content: 'Hello, World!' },
  });
  console.log('Tool call result:', result);
} catch (error) {
  console.error('Tool call failed:', error);
}
```

---

### list_mcp_tools

**Purpose**: Lists all available tools on a specific MCP server.

**Source**: [`src-tauri/src/lib.rs:43-48`](../src-tauri/src/lib.rs#L43-L48)

**Parameters**:

- `server_name: String` - The name of the server.

**Returns**:

- `Result<Vec<MCPTool>, String>` - A list of tool definitions on success, or an error message on failure.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/tauri';

try {
  const tools = await invoke('list_mcp_tools', { serverName: 'filesystem' });
  console.log('Available tools:', tools);
} catch (error) {
  console.error('Failed to list tools:', error);
}
```

---

### list_tools_from_config

**Purpose**: Starts servers from a configuration object and lists all their tools. This is useful for discovering tools from multiple servers at once.

**Source**: [`src-tauri/src/lib.rs:50-142`](../src-tauri/src/lib.rs#L50-L142)

**Parameters**:

- `config: serde_json::Value` - A JSON object containing server configurations. It supports both `mcpServers` (object) and `servers` (array) formats.

**Returns**:

- `Result<Vec<MCPTool>, String>` - A flattened list of all tools from all servers, with tool names prefixed by their server name (e.g., `serverName__toolName`).

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/tauri';

const config = {
  mcpServers: {
    filesystem: {
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-filesystem', '/tmp'],
    },
    // ... other servers
  },
};

try {
  const allTools = await invoke('list_tools_from_config', { config });
  console.log('All discovered tools:', allTools);
} catch (error) {
  console.error('Failed to list tools from config:', error);
}
```

## Status & Monitoring

### get_connected_servers

**Purpose**: Gets a list of all currently connected MCP server names.

**Source**: [`src-tauri/src/lib.rs:144-146`](../src-tauri/src/lib.rs#L144-L146)

**Returns**:

- `Vec<String>` - A list of connected server names.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/tauri';

const connectedServers = await invoke<string[]>('get_connected_servers');
console.log('Connected servers:', connectedServers);
```

---

### check_server_status

**Purpose**: Checks if a specific MCP server is currently alive and responsive.

**Source**: [`src-tauri/src/lib.rs:148-150`](../src-tauri/src/lib.rs#L148-L150)

**Parameters**:

- `server_name: String` - The name of the server to check.

**Returns**:

- `bool` - `true` if the server is alive, `false` otherwise.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/tauri';

const isAlive = await invoke('check_server_status', {
  serverName: 'filesystem',
});
console.log('Server status:', isAlive ? 'Alive' : 'Not Responding');
```

---

### check_all_servers_status

**Purpose**: Checks the status of all managed MCP servers.

**Source**: [`src-tauri/src/lib.rs:152-154`](../src-tauri/src/lib.rs#L152-L154)

**Returns**:

- `std::collections::HashMap<String, bool>` - A map where keys are server names and values are their status (`true` for alive, `false` for not responding).

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/tauri';

const allStatuses = await invoke('check_all_servers_status');
console.log('All server statuses:', allStatuses);
```

## Utility Functions

### list_all_tools

**Purpose**: Lists all tools from all currently connected servers.

**Source**: [`src-tauri/src/lib.rs:156-161`](../src-tauri/src/lib.rs#L156-L161)

**Returns**:

- `Result<Vec<mcp::MCPTool>, String>` - A list of all tools, or an error message.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/tauri';

try {
  const allTools = await invoke('list_all_tools');
  console.log('All tools from connected servers:', allTools);
} catch (error) {
  console.error(error);
}
```

---

### get_validated_tools

**Purpose**: Retrieves a list of tools from a server that have a valid schema.

**Source**: [`src-tauri/src/lib.rs:163-168`](../src-tauri/src/lib.rs#L163-L168)

**Parameters**:

- `server_name: String` - The name of the server.

**Returns**:

- `Result<Vec<mcp::MCPTool>, String>` - A list of validated tools, or an error message.

---

### validate_tool_schema

**Purpose**: Validates the schema of a single tool.

**Source**: [`src-tauri/src/lib.rs:170-173`](../src-tauri/src/lib.rs#L170-L173)

**Parameters**:

- `tool: mcp::MCPTool` - The tool object to validate.

**Returns**:

- `Result<(), String>` - Returns `Ok` if the schema is valid, otherwise an error message.
