# Tauri Commands API Reference

This document provides a comprehensive reference for all Tauri commands available in LibrAgent, grouped by functionality.

## Agent Workflow Commands

Commands for managing agent sessions, messages, and execution flow.

### Session Management

#### `agent_create_session`
Creates a new agent session.
- **Rust Function**: `agent_create_session`
- **Arguments**:
  - `request`: `CreateAgentSessionRequest`
    - `sessionId`: string (UUID)
    - `name`: string | null
    - `model`: string | null
    - `provider`: string | null
    - `agentConfig`: `AgentConfig`
    - `isEphemeral`: boolean (default: false)
    - `workspacePath`: string | null
- **Returns**: `SessionMetadata`

#### `agent_get_session`
Retrieves metadata for a specific session.
- **Rust Function**: `agent_get_session`
- **Arguments**:
  - `sessionId`: string
- **Returns**: `SessionMetadata | null`

#### `agent_get_all_sessions`
Retrieves all stored sessions.
- **Rust Function**: `agent_get_all_sessions`
- **Returns**: `SessionMetadata[]`

#### `agent_delete_session`
Deletes a session and its associated data (messages, memory).
- **Rust Function**: `agent_delete_session`
- **Arguments**:
  - `sessionId`: string
- **Returns**: `AgentResponse`

#### `agent_clear_all_sessions`
Deletes all sessions.
- **Rust Function**: `agent_clear_all_sessions`
- **Returns**: `AgentResponse`

#### `agent_update_session_config`
Updates the configuration for an existing session.
- **Rust Function**: `agent_update_session_config`
- **Arguments**:
  - `request`: `UpdateAgentConfigRequest`
    - `sessionId`: string
    - `model`: string | null
    - `provider`: string | null
    - `agentConfig`: `AgentConfig`
- **Returns**: `AgentResponse`

### Message Handling

#### `agent_send_message`
Sends a user message to an agent session, triggering the workflow.
- **Rust Function**: `agent_send_message`
- **Arguments**:
  - `request`: `SendUserMessageRequest`
    - `sessionId`: string
    - `message`: `AgentMessageDto`
- **Returns**: `AgentResponse`

#### `agent_init_session_with_messages`
Initializes a session with messages from the database (hydrates state).
- **Rust Function**: `agent_init_session_with_messages`
- **Arguments**:
  - `sessionId`: string
- **Returns**: `AgentResponse`

#### `messages_get_page`
Retrieves a paginated list of messages for a session.
- **Rust Function**: `messages_get_page`
- **Arguments**:
  - `sessionId`: string
  - `page`: number
  - `pageSize`: number
- **Returns**: `PaginatedResponse<Message>`

### Workflow Execution

#### `agent_handle_llm_response`
Processes a response received from the LLM (called by frontend).
- **Rust Function**: `agent_handle_llm_response`
- **Arguments**:
  - `sessionId`: string
  - `assistantMessage`: `AgentMessageDto`
- **Returns**: `AgentResponse`

#### `agent_handle_tool_result`
Processes the result of a tool execution (called by frontend).
- **Rust Function**: `agent_handle_tool_result`
- **Arguments**:
  - `sessionId`: string
  - `toolCallId`: string
  - `result`: `ToolExecutionResult`
- **Returns**: `AgentResponse`

#### `agent_handle_llm_error`
Handles errors reported by the LLM provider.
- **Rust Function**: `agent_handle_llm_error`
- **Arguments**:
  - `sessionId`: string
  - `error`: string
- **Returns**: `AgentResponse`

#### `agent_pause_workflow`
Pauses an active workflow.
- **Rust Function**: `agent_pause_workflow`
- **Arguments**:
  - `sessionId`: string
- **Returns**: `AgentResponse`

#### `agent_resume_workflow`
Resumes a paused workflow.
- **Rust Function**: `agent_resume_workflow`
- **Arguments**:
  - `sessionId`: string
- **Returns**: `AgentResponse`

#### `agent_terminate_workflow`
Terminates a running workflow immediately.
- **Rust Function**: `agent_terminate_workflow`
- **Arguments**:
  - `sessionId`: string
- **Returns**: `AgentResponse`

#### `agent_get_available_tools`
Retrieves tools available to a specific session (filtered by config).
- **Rust Function**: `agent_get_available_tools`
- **Arguments**:
  - `sessionId`: string
- **Returns**: `MCPTool[]`

## MCP Management Commands

Commands for managing Model Context Protocol (MCP) servers and tools.

### Server Management

#### `start_mcp_server`
Starts an MCP server.
- **Rust Function**: `start_mcp_server`
- **Arguments**:
  - `config`: `MCPServerConfig`
- **Returns**: `string` (server name)

#### `stop_mcp_server`
Stops a running MCP server.
- **Rust Function**: `stop_mcp_server`
- **Arguments**:
  - `serverName`: string

#### `get_connected_servers`
Lists all currently connected MCP servers.
- **Rust Function**: `get_connected_servers`
- **Returns**: `string[]`

#### `check_server_status`
Checks if a specific server is responsive.
- **Rust Function**: `check_server_status`
- **Arguments**:
  - `serverName`: string
- **Returns**: `boolean`

#### `create_mcp_server_config`
Saves a new MCP server configuration.
- **Rust Function**: `create_mcp_server_config`
- **Arguments**:
  - `config`: `MCPServerConfig`
- **Returns**: `MCPServerConfig`

#### `list_mcp_server_configs`
Lists all saved MCP server configurations.
- **Rust Function**: `list_mcp_server_configs`
- **Returns**: `MCPServerConfig[]`

### Tool Operations

#### `call_mcp_tool`
Calls a tool on a specific MCP server.
- **Rust Function**: `call_mcp_tool`
- **Arguments**:
  - `serverName`: string
  - `toolName`: string
  - `arguments`: object
- **Returns**: `ToolCallResult`

#### `list_mcp_tools`
Lists tools provided by a specific server.
- **Rust Function**: `list_mcp_tools`
- **Arguments**:
  - `serverName`: string
- **Returns**: `MCPTool[]`

#### `list_all_tools`
Lists all tools from all connected servers.
- **Rust Function**: `list_all_tools`
- **Returns**: `MCPTool[]`

## Built-in Tools Commands

Direct access to built-in capabilities.

### Browser

#### `create_browser_session`
Starts a headless browser session.
- **Rust Function**: `create_browser_session`
- **Arguments**:
  - `sessionId`: string (optional)
  - `url`: string (optional)
- **Returns**: `string` (browser session ID)

#### `navigate_to_url`
Navigates a browser session to a URL.
- **Rust Function**: `navigate_to_url`
- **Arguments**:
  - `id`: string (browser session ID)
  - `url`: string

#### `execute_script`
Executes JavaScript in a browser session.
- **Rust Function**: `execute_script`
- **Arguments**:
  - `id`: string
  - `script`: string
- **Returns**: `any`

### File System

#### `list_workspace_files`
Lists files in the current workspace directory.
- **Rust Function**: `list_workspace_files`
- **Arguments**:
  - `path`: string (relative path)
- **Returns**: `FileEntry[]`

#### `read_dropped_file`
Reads the content of a file dropped onto the window.
- **Rust Function**: `read_dropped_file`
- **Arguments**:
  - `filePath`: string
- **Returns**: `string` (file content)

#### `write_file`
Writes content to a file.
- **Rust Function**: `write_file`
- **Arguments**:
  - `path`: string
  - `content`: string

## System Commands

#### `get_app_logs_dir`
Returns the directory path where application logs are stored.
- **Rust Function**: `get_app_logs_dir`
- **Returns**: `string`

#### `list_log_files`
Lists available log files.
- **Rust Function**: `list_log_files`
- **Returns**: `string[]`

#### `set_setting`
Updates a system setting.
- **Rust Function**: `set_setting`
- **Arguments**:
  - `key`: string
  - `value`: string
- **Returns**: `void`

#### `get_setting`
Retrieves a system setting.
- **Rust Function**: `get_setting`
- **Arguments**:
  - `key`: string
- **Returns**: `string | null`

#### `agent_factory_reset`
Resets all data and settings to factory defaults.
- **Rust Function**: `agent_factory_reset`
- **Returns**: `AgentResponse`

---
**Note**: All commands are invoked using `invoke('command_name', { ...args })` from the frontend. Argument keys must match the camelCase names listed above.
