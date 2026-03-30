# Tauri Commands

This document provides a comprehensive reference for the primary Tauri
commands available in LibrAgent. Commands are grouped by domain and are
located in `src-tauri/src/commands/`.

## Agent Workflow Management

### agent_create_session

**Purpose**: Creates a new agent session to begin a conversation workflow.

**Source**: `src-tauri/src/commands/agent_commands.rs`

**Parameters**:

- `request: CreateAgentSessionRequest` - The session configuration object.
  - `session_id: String` - Unique identifier for the session.
  - `name: Option<String>` - Optional descriptive name.
  - `model: Option<String>` - LLM model ID.
  - `provider: Option<String>` - LLM provider name.
  - `agent_config: AgentConfig` - System prompt and tool settings.
  - `is_ephemeral: bool` - If true, data is not persisted (defaults to false).
  - `workspace_path: Option<String>` - Custom path for session files.

**Returns**:

- `Result<SessionMetadata, String>` - Returns the created session
  metadata on success.

---

### agent_send_message

**Purpose**: Sends a new user message to an active agent session, triggering
the agent's workflow (LLM completion and tool execution).

**Source**: `src-tauri/src/commands/agent_commands.rs`

**Parameters**:

- `request: SendUserMessageRequest` - The message payload.
  - `session_id: String` - Active session ID.
  - `message: Message` - Message object.

**Returns**:

- `Result<AgentResponse, String>` - Returns status success/message
  on workflow start.

---

### agent_delete_session

**Purpose**: Deletes a session and its child sessions.

**Source**: `src-tauri/src/commands/agent_commands.rs`

**Parameters**:

- `session_id: String` - Active session ID.

**Returns**:

- `Result<AgentResponse, String>` - Returns success status.

## Workspace & Files

### list_workspace_files

**Purpose**: Lists all files and directories in the session's workspace.

**Source**: `src-tauri/src/commands/workspace_commands.rs`

**Parameters**:

- `path: Option<String>` - Optional relative path within the workspace.
- `session_id: Option<String>` - The session ID to list files for.

**Returns**:

- `Result<Vec<WorkspaceFileItem>, String>` - Returns a tree of files.

---

### workspace_write_file

**Purpose**: Writes content to a file in the session's workspace.

**Source**: `src-tauri/src/commands/file_commands.rs`

**Parameters**:

- `file_path: String` - The relative file path.
- `content: Vec<u8>` - The content to write.
- `session_id: Option<String>` - The session ID.

**Returns**:

- `Result<(), String>` - Returns success or error.

## Browser Automation

### create_browser_session

**Purpose**: Initializes a new real-time browser session using Tauri Webview.

**Source**: `src-tauri/src/commands/browser_commands.rs`

**Parameters**:

- `url: String` - Initial URL to load.
- `title: Option<String>` - Optional title for the browser session.

**Returns**:

- `Result<CreateSessionResponse, String>` - Session info.

---

### navigate_to_url

**Purpose**: Navigates an active browser session to a new URL.

**Source**: `src-tauri/src/commands/browser_commands.rs`

**Parameters**:

- `session_id: String` - The browser session ID.
- `url: String` - The URL to navigate to.

**Returns**:

- `Result<String, String>` - Returns success message or error.

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

**Purpose**: Retrieves a list of all built-in MCP servers available in
LibrAgent (e.g., `planning`, `workspace`).

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

**Purpose**: Retrieves a list of tool definitions for a built-in server.
_Note: Currently returns an empty list as tool definitions are managed
dynamically per session._

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

- `Result<Option<SettingDto>, String>` - Returns setting if exists, or `null`.

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
