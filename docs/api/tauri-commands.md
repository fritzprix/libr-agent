# Tauri Commands

This document provides a comprehensive reference for the primary Tauri commands
available in LibrAgent. Commands are grouped by domain and are located in
`src-tauri/src/commands/`.

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

**Purpose**: Sends a new user message to an active agent session, triggering the
agent's workflow (LLM completion and tool execution).

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

**Purpose**: Cleans up a session workspace, search index, and metadata. _Note:
Does not delete session records from the primary database._

**Source**: `src-tauri/src/commands/session_commands.rs`

**Parameters**:

- `sessionId: String` - The unique identifier of the session to clean up.

**Returns**:

- `Result<SessionResponse, String>` - Returns a status wrapper confirming cleanup
  on success.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  await invoke('remove_session', { sessionId: 'session-123' });
} catch (error) {
  console.error('Failed to remove session:', error);
}
```

## Workspace Management

### list_workspace_files

**Purpose**: Lists files and directories in the current session's workspace.

**Source**: `src-tauri/src/commands/workspace_commands.rs`

**Parameters**:

- `path: Option<String>` - Optional relative path within the workspace to list.
  If null, lists the root.
- `sessionId: Option<String>` - Optional session ID to target. If null, targets
  the global workspace.

**Returns**:

- `Result<Vec<WorkspaceFileDto>, String>` - Returns a list of file details.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  const files = await invoke('list_workspace_files', {
    path: null,
    sessionId: 'session-123',
  });
  console.log('Workspace files:', files);
} catch (error) {
  console.error('Failed to list files:', error);
}
```

---

### get_workspace_dir

**Purpose**: Returns the absolute workspace directory path for the given
session.

**Source**: `src-tauri/src/commands/workspace_commands.rs`

**Parameters**:

- `sessionId: String` - The ID of the session.

**Returns**:

- `Result<String, String>` - Returns the absolute path to the workspace directory.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  const dir = await invoke('get_workspace_dir', { sessionId: 'session-123' });
  console.log('Workspace directory:', dir);
} catch (error) {
  console.error('Failed to get directory:', error);
}
```

---

### open_workspace_in_explorer

**Purpose**: Opens the workspace directory in the system's default file
explorer.

**Source**: `src-tauri/src/commands/workspace_commands.rs`

**Parameters**:

- `sessionId: String` - The ID of the session.

**Returns**:

- `Result<(), String>` - Returns success or error message.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  await invoke('open_workspace_in_explorer', { sessionId: 'session-123' });
} catch (error) {
  console.error('Failed to open explorer:', error);
}
```

---

### open_workspace_in_terminal

**Purpose**: Opens the workspace directory in the system's default terminal.

**Source**: `src-tauri/src/commands/workspace_commands.rs`

**Parameters**:

- `sessionId: String` - The ID of the session.

**Returns**:

- `Result<(), String>` - Returns success or error message.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  await invoke('open_workspace_in_terminal', { sessionId: 'session-123' });
} catch (error) {
  console.error('Failed to open terminal:', error);
}
```

## Browser Automation

### create_browser_session

**Purpose**: Initializes a new interactive browser session for agent automation.

**Source**: `src-tauri/src/commands/browser_commands.rs`

**Parameters**:

- `sessionId: String` - The session ID to link the browser to.

**Returns**:

- `Result<BrowserSessionResponse, String>` - Returns the newly created browser
  session metadata.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  const session = await invoke('create_browser_session', {
    sessionId: 'session-123',
  });
  console.log('Browser session created:', session);
} catch (error) {
  console.error('Failed to create browser session:', error);
}
```

---

### close_browser_session

**Purpose**: Closes an active browser session and cleans up resources.

**Source**: `src-tauri/src/commands/browser_commands.rs`

**Parameters**:

- `sessionId: String` - The browser session ID to close.

**Returns**:

- `Result<String, String>` - Returns a success message.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  await invoke('close_browser_session', { sessionId: 'session-123' });
} catch (error) {
  console.error('Failed to close browser session:', error);
}
```

---

### list_browser_sessions

**Purpose**: Retrieves a list of all active browser sessions.

**Source**: `src-tauri/src/commands/browser_commands.rs`

**Returns**:

- `Result<Vec<BrowserSession>, String>` - Returns a list of browser session objects.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  const sessions = await invoke('list_browser_sessions');
  console.log('Active browser sessions:', sessions);
} catch (error) {
  console.error('Failed to list browser sessions:', error);
}
```

---

### navigate_to_url

**Purpose**: Instructs the browser session to navigate to a specific URL.

**Source**: `src-tauri/src/commands/browser_commands.rs`

**Parameters**:

- `sessionId: String` - The ID of the browser session.
- `url: String` - The URL to navigate to.

**Returns**:

- `Result<String, String>` - Returns a success message.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  await invoke('navigate_to_url', {
    sessionId: 'session-123',
    url: 'https://example.com',
  });
} catch (error) {
  console.error('Failed to navigate:', error);
}
```

---

### execute_script

**Purpose**: Executes a JavaScript snippet in the context of the active browser
session.

**Source**: `src-tauri/src/commands/browser_commands.rs`

**Parameters**:

- `sessionId: String` - The ID of the browser session.
- `script: String` - The JavaScript code to execute.

**Returns**:

- `Result<String, String>` - Returns the result of the script execution as a string.

**Usage**:

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  const result = await invoke('execute_script', {
    sessionId: 'session-123',
    script: 'document.title',
  });
  console.log('Script result:', result);
} catch (error) {
  console.error('Failed to execute script:', error);
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

**Purpose**: Retrieves a list of all built-in MCP servers available in LibrAgent
(e.g., `planning`, `workspace`).

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

**Purpose**: Retrieves a list of tool definitions for a built-in server. _Note:
Currently returns an empty list as tool definitions are managed dynamically per
session._

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
