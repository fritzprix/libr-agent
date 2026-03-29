# Tauri Commands

This document provides a comprehensive reference for the primary Tauri commands available in LibrAgent. Commands are grouped by domain and are located in `src-tauri/src/commands/`.

## Agent Workflow Management

### agent_create_session

**Purpose**: Creates a new agent session to begin a conversation workflow.

**Source**: `src-tauri/src/commands/agent_commands.rs`

**Parameters**:

- `request: CreateAgentSessionRequest` - The session configuration object.
  - `sessionId: String` - Unique identifier for the session.
  - `name: Option<String>` - Optional descriptive name.
  - `model: Option<String>` - LLM model ID.
  - `provider: Option<String>` - LLM provider name.
  - `agentConfig: AgentConfig` - System prompt and tool settings.
  - `isEphemeral: bool` - If true, data is not persisted (defaults to false).
  - `workspacePath: Option<String>` - Custom path for session files.

**Returns**:

- `Result<SessionMetadata, String>` - Returns the created session metadata on success.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  const session = await invoke('agent_create_session', {
    request: {
      sessionId: 'session-123',
      agentConfig: {
        systemPrompt: 'You are a helpful assistant.',
        tools: [],
      },
    },
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

- `request: SendUserMessageRequest` - The message payload.
  - `sessionId: String` - Active session ID.
  - `message: Message` - Message object with `id`, `role`, and `content`.

**Returns**:

- `Result<AgentResponse, String>` - Returns status success/message on workflow start.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  const response = await invoke('agent_send_message', {
    request: {
      sessionId: 'session-123',
      message: {
        id: 'msg-1',
        role: 'user',
        content: [{ type: 'text', text: 'List the files in /tmp' }],
      },
    },
  });
  console.log('Workflow started:', response);
} catch (error) {
  console.error('Failed to send message:', error);
}
```

## Session Management

### remove_session

**Purpose**: Cleans up a session workspace, search index, and metadata. _Note: Does not delete session records from the primary database._

**Source**: `src-tauri/src/commands/session_commands.rs`

**Parameters**:

- `sessionId: String` - The unique identifier of the session to clean up.

**Returns**:

- `Result<SessionResponse, String>` - Returns a status wrapper confirming cleanup on success.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

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
import { invoke } from '@tauri-apps/api/core';

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

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

const servers = await invoke<string[]>('list_builtin_servers');
```

---

### list_builtin_tools

**Purpose**: Retrieves a list of tool definitions for a built-in server. _Note: Currently returns an empty list as tool definitions are managed dynamically per session._

**Source**: `src-tauri/src/commands/mcp_commands.rs`

**Parameters**:

- `server_name: Option<String>` - (Optional) The name of the built-in server.

**Returns**:

- `Vec<MCPTool>` - A list of tools (currently empty).

---

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
import { invoke } from '@tauri-apps/api/core';

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

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  const theme = await invoke('get_setting', { key: 'theme' });
  console.log('Current theme:', theme);
} catch (error) {
  console.error('Failed to get setting:', error);
}
```
