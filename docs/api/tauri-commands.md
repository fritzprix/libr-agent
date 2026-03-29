# Tauri Commands

This document provides a comprehensive reference for the primary Tauri commands available in LibrAgent. Commands are grouped by domain and are located in `src-tauri/src/commands/`.

## Agent Workflow Management

### agent_create_session

**Purpose**: Creates a new agent session to begin a conversation workflow.

**Source**: `src-tauri/src/commands/agent_commands.rs`

**Parameters**:
- `assistant_id: String` - The ID of the assistant profile to use.
- `name: Option<String>` - An optional name for the session.

**Returns**:
- `Result<SessionResponse, String>` - Returns the created session metadata on success, or an error message on failure.

**Usage**:
```typescript
import { invoke } from '@tauri-apps/api/tauri';

try {
  const session = await invoke('agent_create_session', {
    assistantId: 'asst_coder',
    name: 'Debug Session'
  });
  console.log('Session created:', session);
} catch (error) {
  console.error('Failed to create session:', error);
}
```

---

### agent_send_message

**Purpose**: Sends a new user message to an active agent session, triggering the agent's workflow (LLM completion and tool execution).

**Source**: `src-tauri/src/commands/agent_commands.rs`

**Parameters**:
- `request: SendMessageRequest` - The message payload, including `session_id` and the `content` block.

**Returns**:
- `Result<(), String>` - Returns `Ok` when the workflow starts, or an error message if the session cannot be found or is busy.

**Usage**:
```typescript
import { invoke } from '@tauri-apps/api/tauri';

try {
  await invoke('agent_send_message', {
    request: {
      session_id: 'session-123',
      content: [{ type: 'text', text: 'List the files in /tmp' }]
    }
  });
} catch (error) {
  console.error('Failed to send message:', error);
}
```

## Session Management

### remove_session

**Purpose**: Permanently deletes an agent session and its associated messages.

**Source**: `src-tauri/src/commands/session_commands.rs`

**Parameters**:
- `session_id: String` - The unique identifier of the session to delete.

**Returns**:
- `Result<SessionResponse, String>` - Returns the deleted session metadata on success.

**Usage**:
```typescript
import { invoke } from '@tauri-apps/api/tauri';

try {
  await invoke('remove_session', { sessionId: 'session-123' });
} catch (error) {
  console.error('Failed to remove session:', error);
}
```

## MCP Integration

### probe_mcp_server

**Purpose**: Probes an MCP server to retrieve its list of available tools.

**Source**: `src-tauri/src/commands/mcp_commands.rs`

**Parameters**:
- `server_id: String` - The ID of the server to probe.

**Returns**:
- `Result<Vec<MCPTool>, String>` - A list of tools available on the server.

**Usage**:
```typescript
import { invoke } from '@tauri-apps/api/tauri';

try {
  const tools = await invoke('probe_mcp_server', { serverId: 'filesystem' });
  console.log('Available tools:', tools);
} catch (error) {
  console.error('Failed to probe server:', error);
}
```

---

### list_builtin_servers

**Purpose**: Retrieves a list of all built-in MCP servers available in LibrAgent (e.g., `planning`, `workspace`).

**Source**: `src-tauri/src/commands/mcp_commands.rs`

**Returns**:
- `Vec<String>` - A list of built-in server names.

---

### list_builtin_tools

**Purpose**: Retrieves a list of tools for a specific built-in server or all built-in servers if none is specified.

**Source**: `src-tauri/src/commands/mcp_commands.rs`

**Parameters**:
- `server_name: Option<String>` - (Optional) The name of the built-in server.

**Returns**:
- `Vec<MCPTool>` - A list of tools.

## System Settings

### set_setting

**Purpose**: Updates or creates a global application setting.

**Source**: `src-tauri/src/commands/settings_commands.rs`

**Parameters**:
- `key: String` - The configuration key (e.g., `theme`, `llm_provider`).
- `value: Value` - The JSON value for the setting.

**Returns**:
- `Result<SettingDto, String>` - Returns the updated setting on success.

**Usage**:
```typescript
import { invoke } from '@tauri-apps/api/tauri';

try {
  await invoke('set_setting', { key: 'theme', value: 'dark' });
} catch (error) {
  console.error('Failed to update setting:', error);
}
```

---

### get_setting

**Purpose**: Retrieves the value of a specific global application setting.

**Source**: `src-tauri/src/commands/settings_commands.rs`

**Parameters**:
- `key: String` - The configuration key.

**Returns**:
- `Result<Option<SettingDto>, String>` - Returns the setting if it exists, or `null`.

