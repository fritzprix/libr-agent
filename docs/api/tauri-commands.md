# Tauri Commands Reference

This document provides a comprehensive reference and master catalog for the **186 Tauri commands** registered in LibrAgent (`src-tauri/src/lib.rs`).
Commands are invoked from the frontend React UI or test harnesses using Tauri's IPC bridge:

```typescript
import { invoke } from '@tauri-apps/api/core';
```

---

## 1. Master Command Directory by Domain

| Domain | Count | Key Commands Included |
|---|---|---|
| [1. Agent Session & Workflow](#1-agent-session--workflow-commands) | 50 | `agent_create_session`, `agent_send_message`, `agent_call_builtin_tool`, `agent_respond_tool_approval`, `agent_get_child_sessions`, `agent_terminate_workflow`, etc. |
| [2. Messages & Conversation History](#2-messages--conversation-history) | 7 | `messages_get_page`, `messages_get_messages_before`, `messages_upsert_many`, `messages_search`, etc. |
| [3. Assistant Configuration](#3-assistant-configuration) | 8 | `create_assistant`, `update_assistant`, `get_assistant`, `list_assistant_summaries`, `search_assistants`, etc. |
| [4. MCP Servers & Tools](#4-mcp-servers--tools) | 9 | `probe_mcp_server`, `validate_tool_schema`, `list_builtin_servers`, `list_mcp_server_presets`, `create_mcp_server_config`, etc. |
| [5. Skills Management](#5-skills-management) | 18 | `scan_skills_directory`, `get_managed_skills_overview`, `import_user_skills`, `install_github_skills`, `copy_global_to_assistant`, etc. |
| [6. Interactive Browser & Extension Bridge](#6-interactive-browser--extension-bridge) | 10 | `create_browser_session`, `navigate_to_url`, `execute_script`, `get_extension_bridge_status`, `clear_agent_browser_data`, etc. |
| [7. Scheduled Tasks & Automation](#7-scheduled-tasks--automation) | 9 | `create_scheduled_task`, `list_scheduled_tasks`, `toggle_scheduled_task`, `list_session_scheduled_tasks`, etc. |
| [8. Playbooks & Templates](#8-playbooks--templates) | 6 | `create_playbook`, `update_playbook`, `get_playbook`, `list_playbooks`, `toggle_playbook_bookmark`, etc. |
| [9. Downloads & Session Export](#9-downloads--session-export) | 6 | `download_media_file`, `download_text_pdf`, `export_session_file`, `export_and_download_zip`, etc. |
| [10. Knowledge Base](#10-knowledge-base) | 4 | `list_global_knowledge`, `get_global_knowledge_detail`, `get_global_knowledge_graph`, `delete_global_knowledge` |
| [11. Workspace & Filesystem](#11-workspace--filesystem) | 16 | `list_workspace_files`, `workspace_write_file`, `open_workspace_in_terminal`, `set_workspace_override`, `submit_interactive_shell_input`, etc. |
| [12. OAuth 2.1 Authentication](#12-oauth-21-authentication) | 4 | `start_oauth_flow`, `get_oauth_token`, `has_oauth_token`, `revoke_oauth_token` |
| [13. System Settings & Migration](#13-system-settings--migration) | 9 | `get_setting`, `set_setting`, `export_migration`, `import_migration`, `inspect_migration`, `reverify_mcp_servers`, etc. |
| [14. Diagnostics, Logs & Media Assist](#14-diagnostics-logs--media-assist) | 18 | `media_assist_plugin_status`, `media_assist_run_plugin`, `probe_runtime_binaries`, `check_docker_health`, `log_batch`, `restart_app`, etc. |
| **Total Registered Commands** | **186** | |

---

## 2. Command Specifications by Domain

### 1. Agent Session & Workflow Commands

Implements the core agent runtime loop, session branching, LLM streaming callbacks, and human-in-the-loop approvals. Located in `src-tauri/src/commands/agent_commands/`.

#### Catalog
- **Session Lifecycle**: `agent_create_session`, `agent_create_session_with_initial_message`, `agent_resume_session`, `agent_open_session`, `agent_init_session_with_messages`, `agent_get_session`, `agent_get_all_sessions`, `agent_list_sessions`, `agent_list_attention_sessions`, `agent_delete_session`, `agent_delete_session_only`, `remove_session`, `agent_clear_all_sessions`, `agent_factory_reset`.
- **Workflow Execution**: `agent_send_message`, `agent_execute_command`, `agent_pause_workflow`, `agent_resume_workflow`, `agent_terminate_workflow`, `agent_cancel_workflow`, `agent_set_execution_mode`.
- **LLM & Tool Handling**: `agent_handle_llm_response`, `agent_handle_llm_error`, `agent_report_llm_streaming_issue`, `agent_handle_tool_result`, `agent_call_builtin_tool`, `agent_respond_tool_approval`.
- **Subagent & Session Tree**: `agent_get_child_session_ids`, `agent_get_child_sessions`, `agent_get_descendant_session_ids`.
- **Attachments & Context**: `agent_add_attachment`, `agent_delete_attachment`, `delete_attachments`, `agent_get_service_contexts`, `agent_get_compact_context`, `agent_handle_compact_response`, `agent_handle_compact_error`.
- **Channels & Queue**: `agent_inject_messages`, `agent_append_tool_messages`, `agent_get_pending_queue`, `agent_cancel_pending_prompt`, `agent_inject_channel_message`, `agent_inject_channel_message_auto`, `agent_respond_channel_permission`.
- **Metadata & View State**: `agent_update_session_config`, `agent_toggle_session_bookmark`, `agent_update_session_name`, `agent_mark_session_viewed`, `agent_execute_ui_tauri_action`.

#### Key Signatures

##### `agent_create_session`
Creates a new conversation session with assistant binding, optional ephemeral mode, and custom workspace.

```typescript
const session = await invoke('agent_create_session', {
  request: {
    sessionId: 'session-unique-id',
    name: 'Refactor Auth Service',
    model: 'claude-3-7-sonnet',
    provider: 'anthropic',
    agentConfig: { assistantId: 'assistant-uuid' },
    isEphemeral: false,
    workspacePath: '/path/to/project',
  },
});
```

##### `agent_send_message`
Submits a user message and triggers the background agent loop (LLM turn + tool iterations).

```typescript
await invoke('agent_send_message', {
  request: {
    sessionId: 'session-unique-id',
    message: {
      id: 'msg-1',
      role: 'user',
      content: [{ type: 'text', text: 'Run the test suite.' }],
    },
  },
});
```

##### `agent_respond_tool_approval`
Submits user approval or rejection for a tool execution requiring confirmation (YOLO mode gate).

```typescript
await invoke('agent_respond_tool_approval', {
  request: {
    sessionId: 'session-unique-id',
    toolCallId: 'call-xyz',
    approved: true,
    rejectionReason: null,
  },
});
```

---

### 2. Messages & Conversation History

Manages durable message persistence in SQLite. Located in `src-tauri/src/commands/message_commands.rs`.

- `messages_get_page`: Paginates session messages (`pageSize`, `beforeRowId`).
- `messages_get_messages_before`: Loads causal history prior to a reference message.
- `messages_upsert`: Inserts or updates an individual message.
- `messages_upsert_many`: Bulk persists messages inside a single transaction.
- `messages_delete`: Deletes a specific message.
- `messages_delete_all_for_session`: Cleans all messages for a session.
- `messages_search`: Full-text search over message contents across sessions.

---

### 3. Assistant Configuration

Manages persistent assistant personas, prompt templates, and tool bindings. Located in `src-tauri/src/commands/assistant_commands.rs`.

- `create_assistant`: Registers a new assistant profile.
- `update_assistant`: Modifies an existing assistant's system prompt, provider, or assigned MCP servers.
- `delete_assistant`: Deletes an assistant.
- `get_assistant`: Retrieves full assistant details by ID.
- `list_assistants`: Returns all configured assistants.
- `list_assistant_summaries`: Returns lightweight summaries for UI selector dropdowns.
- `search_assistants`: Searches assistants by name, description, or tag.
- `batch_upsert_assistants`: Bulk imports assistant configurations.

---

### 4. MCP Servers & Tools

Manages external stdio MCP processes and internal builtin servers. Located in `src-tauri/src/commands/mcp_commands.rs`.

- `probe_mcp_server`: Tests connectivity to an external MCP server and discovers tool schemas.
- `validate_tool_schema`: Validates a JSON schema definition against tool invocation requirements.
- `list_builtin_servers`: Returns names of active built-in servers.
- `list_builtin_tools`: Returns tool definitions for built-in services.
- `list_builtin_servers_with_metadata`: Returns built-in servers with status and metadata.
- `list_available_builtin_server_definitions`: Returns all canonical built-in server blueprints.
- `list_mcp_server_presets`: Retrieves pre-configured MCP presets (GitHub, Brave Search, etc.).
- `create_mcp_server_config`: Saves a new external MCP server configuration.
- `update_mcp_server_config`: Updates environment variables or command parameters.
- `delete_mcp_server_config`: Removes an MCP server definition.
- `list_mcp_server_configs`: Returns all configured external MCP servers.

---

### 5. Skills Management

Manages modular agent capabilities across workspace, user, and assistant scopes. Located in `src-tauri/src/commands/skills_commands.rs`.

- `scan_skills_directory`: Scans a directory for valid `SKILL.md` bundles.
- `get_default_skills_directory`: Returns path to the system default skills directory (`.agents/skills`).
- `open_skills_directory_in_explorer`: Opens the skills directory in the native file browser.
- `get_aggregated_skills`: Merges and resolves skills active for the current context.
- `get_managed_skills_overview`: Returns a high-level summary of all installed skills.
- `get_skill_content`: Reads the raw Markdown instructions and metadata of a skill.
- `copy_global_to_assistant`: Copies a workspace or global skill into an assistant's private skill set.
- `delete_assistant_skill`: Deletes an assistant-scoped skill.
- `import_assistant_skills`: Imports skill files directly into an assistant.
- `preview_user_skill_import`: Previews metadata before importing a user skill bundle.
- `import_user_skills`: Installs a skill into the user profile.
- `preview_github_skill_install`: Fetches and previews a remote skill repository from GitHub.
- `install_github_skills`: Downloads and installs skills directly from a GitHub repository.
- `delete_user_skill`: Deletes an installed user skill.
- `reset_user_skills`: Resets user skills back to initial factory defaults.
- `reset_assistant_skills`: Resets an assistant's skills back to its template.

---

### 6. Interactive Browser & Extension Bridge

Controls the headless or interactive WebKit/Chromium browser and the Chrome Web Store extension bridge. Located in `src-tauri/src/commands/browser_commands.rs`.

- `create_browser_session`: Launches a new browser automation context.
- `close_browser_session`: Terminates a running browser session.
- `list_browser_sessions`: Lists all open browser windows and tabs.
- `navigate_to_url`: Directs the browser page to a specific URL.
- `execute_script`: Evaluates JavaScript within the target tab.
- `navigate_back`: Simulates history back navigation.
- `navigate_forward`: Simulates history forward navigation.
- `clear_agent_browser_data`: Clears browsing caches, cookies, and local storage.
- `get_extension_bridge_status`: Checks if the LibrAgent Chrome extension is connected and healthy.
- `get_extension_unpacked_path`: Returns the local filesystem path to the unpacked browser bridge extension.

---

### 7. Scheduled Tasks & Automation

Cron and timer orchestration for recurring agent jobs. Located in `src-tauri/src/commands/scheduled_task_commands.rs`.

- `create_scheduled_task`: Registers a new scheduled task (cron or interval-based).
- `list_scheduled_tasks`: Returns all globally scheduled tasks.
- `get_scheduled_task`: Retrieves task status and history.
- `update_scheduled_task`: Updates schedule or prompt definition.
- `toggle_scheduled_task`: Enables or pauses a scheduled task.
- `delete_scheduled_task`: Removes a scheduled task.
- `list_session_scheduled_tasks`: Lists tasks associated with a specific active session.
- `toggle_session_scheduled_task`: Pauses or resumes a session-bound task.
- `cancel_session_scheduled_task`: Cancels a running recurring task for a session.

---

### 8. Playbooks & Templates

Reusable workflow recipes and multi-step prompt templates. Located in `src-tauri/src/commands/playbook_commands.rs`.

- `create_playbook`: Creates a new playbook entry.
- `update_playbook`: Updates playbook title, description, or step definitions.
- `delete_playbook`: Deletes a playbook.
- `get_playbook`: Retrieves full playbook details.
- `list_playbooks`: Lists all saved playbooks.
- `toggle_playbook_bookmark`: Stars or un-stars a playbook for quick access.

---

### 9. Downloads & Session Export

File export and benchmark trajectory generation. Located in `src-tauri/src/commands/download_commands.rs` and `session_export_commands.rs`.

- `download_media_file`: Triggers native file download for inline media assets.
- `download_text_file`: Saves text content to disk via a file dialog.
- `download_binary_file`: Saves binary buffers to disk.
- `download_text_pdf`: Converts and saves text or markdown as a formatted PDF.
- `download_workspace_file`: Copies a file from the workspace to a user-selected destination.
- `export_and_download_zip`: Packages workspace files into a `.zip` archive.
- `export_session_file`: Exports session trajectory in **ATIF v1.7** JSON or **Markdown** format (see [Session Export Formats](../guides/session-export-formats.md)).
- `export_dataset`: Exports messages into an evaluation training dataset.

---

### 10. Knowledge Base

Global document indexing and semantic retrieval. Located in `src-tauri/src/commands/knowledge_commands.rs`.

- `list_global_knowledge`: Lists all indexed knowledge documents.
- `get_global_knowledge_detail`: Retrieves document chunks, metadata, and embeddings.
- `get_global_knowledge_graph`: Retrieves graph relationships between knowledge entities.
- `delete_global_knowledge`: Deletes a document from the knowledge index.

---

### 11. Workspace & Filesystem

Workspace file inspection, path overrides, and native OS app openers. Located in `src-tauri/src/commands/workspace_commands.rs`.

- `list_workspace_files`: Lists files in the current active workspace.
- `list_workspace_file_paths`: Returns recursive file path listing.
- `list_workspace_file_paths_for_path`: Returns file paths under a specific subdirectory.
- `read_workspace_file_content`: Reads text content of a workspace file.
- `read_local_file_as_base64`: Reads any local file as a Base64 string for image/binary previews.
- `write_file`: Writes content to a path.
- `workspace_write_file`: Writes content safely constrained within the active workspace.
- `open_workspace_file_with_default_app`: Launches OS default viewer (e.g., image editor or PDF viewer).
- `open_workspace_in_explorer`: Opens the workspace directory in File Explorer / Finder / file manager.
- `open_workspace_in_terminal`: Opens a native terminal shell inside the workspace directory.
- `open_path_with_default_app`: Launches system handler for any given local path.
- `open_external_url`: Opens a URL in the user's default web browser.
- `get_workspace_dir`: Returns the resolved root path of the current session workspace.
- `get_workspace_override`: Queries whether an active directory override is in effect.
- `set_workspace_override`: Temporarily redirects session workspace to another directory.
- `cancel_workspace_override`: Reverts workspace redirection.
- `submit_interactive_shell_input`: Submits stdin to an interactive terminal session.
- `cancel_interactive_shell_input`: Cancels a pending interactive shell session.

---

### 12. OAuth 2.1 Authentication

OAuth token lifecycle for external integrations. Located in `src-tauri/src/commands/oauth_commands.rs`.

- `start_oauth_flow`: Triggers browser-based OAuth PKCE authorization for a provider.
- `has_oauth_token`: Checks whether a valid access token is stored for a provider.
- `get_oauth_token`: Retrieves the stored token (refreshes expired tokens automatically).
- `revoke_oauth_token`: Deletes and revokes credentials for a provider.

---

### 13. System Settings & Migration

Global app preferences and SQLite database migrations. Located in `src-tauri/src/commands/settings_commands.rs` and `migration_commands.rs`.

- `get_setting`: Retrieves a single global setting value by key.
- `set_setting`: Sets a single global setting value.
- `update_settings`: Batch updates multiple settings inside a single transaction.
- `delete_setting`: Resets a setting to default.
- `list_settings`: Returns all configured key-value settings.
- `export_migration`: Exports the entire SQLite database into a portable backup archive.
- `inspect_migration`: Analyzes a migration file before importing.
- `import_migration`: Restores database records and settings from a backup archive.
- `reverify_mcp_servers`: Re-runs health checks and updates metadata on all configured MCP servers.

---

### 14. Diagnostics, Logs & Media Assist

Host status, logging pipeline, runtime binary probes, and MediaAssist plugins. Located in `src-tauri/src/commands/media_assist_commands.rs`, `log_commands.rs`, and `system_commands.rs`.

- `media_assist_plugin_status`: Checks whether the host MediaAssist plugin is installed and ready (see [MediaAssist Architecture](../architecture/media-assist-architecture.md)).
- `media_assist_run_plugin`: Runs the sandboxed MediaAssist plugin against audio/image/video inputs.
- `probe_runtime_binaries`: Checks whether system dependencies (`node`, `python`, `uv`, `docker`) are available in `$PATH`.
- `check_docker_health`: Verifies Docker daemon status.
- `start_docker_desktop`: Launches Docker Desktop if installed.
- `docker_desktop_launch_supported`: Checks if Docker Desktop launch automation is supported on current OS.
- `get_app_data_dir`: Returns the system path to LibrAgent app data directory.
- `get_app_logs_dir`: Returns the system path to application log files.
- `get_update_install_capability`: Checks if automatic app update installation is supported.
- `backup_current_log`: Rotates and archives the current log file.
- `clear_current_log`: Truncates the active log file.
- `list_log_files`: Lists all rotated log archives.
- `get_launch_log_level`: Returns the current logging verbosity (`trace`, `debug`, `info`, `warn`, `error`).
- `log_trace` / `log_debug` / `log_info` / `log_warn` / `log_error_from_frontend`: Frontend logging sinks that pipe into the unified Rust tracing subscriber.
- `log_batch`: Flushes a batch of frontend telemetry events.
- `greet`: Handshake ping command used for IPC latency testing.
- `restart_app`: Triggers graceful shutdown and restarts the desktop application.
