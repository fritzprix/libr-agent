# LibrAgent — Codebase Feature Map

> Maintained architecture reference based on the current repository layout.
> Every feature listed here should map to real paths in the tree.

---

## 1. Agent Orchestration Engine (Rust-Backed)

The core Think-Act-Observe loop managed entirely in Rust.

| Component               | File                                             | Description                                                                                          |
| ----------------------- | ------------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| **AgentSessionManager** | `src-tauri/src/agent/session_manager.rs` (28 KB) | Lifecycle: create, recover, delete sessions. Concurrency gate, session bus, fan-out management       |
| **Workflow Loop**       | `src-tauri/src/agent/workflow/`                  | Think → Act (tool call) → Observe → Next iteration. Handles LLM response, tool result, errors        |
| **LLM Interaction**     | `src-tauri/src/agent/llm/`                       | Builds messages from conversation history, sends to model, handles streaming/non-streaming responses |
| **Compact Recovery**    | `src-tauri/src/agent/compact_recovery.rs`        | Context compaction: when conversation grows too large, generates summary and prunes history          |
| **Tool Execution**      | `src-tauri/src/agent/tools.rs` (28 KB)           | Dispatches tool calls to MCPServiceProxy, handles result formatting                                  |
| **Tool Approvals**      | `src-tauri/src/agent/tool_approvals.rs`          | Human-in-the-loop approval gates for sensitive operations                                            |
| **Concurrency Control** | `src-tauri/src/agent/concurrency.rs` (10 KB)     | Global concurrency gate, limits parallel agent sessions                                              |
| **Session Bus**         | `src-tauri/src/agent/session_bus.rs` (10 KB)     | Channel-based messaging between session lifecycle and workflow threads                               |
| **Channel Routing**     | `src-tauri/src/agent/channel_routing.rs`         | Routes messages to correct session channel                                                           |
| **Yolo Mode**           | `agent_set_yolo_mode`                            | Bypass tool approval gates for trusted operations                                                    |
| **State Machine**       | `src-tauri/src/agent/state.rs`                   | Session state transitions (Idle → Running → Paused → Completed → Error)                              |

### Frontend Hooks

| File                                                        | Description                                                   |
| ----------------------------------------------------------- | ------------------------------------------------------------- |
| `src/features/agent/hooks/useAgentDraftChat.ts` (20 KB)     | Main chat hook: send messages, handle streaming, manage state |
| `src/features/agent/hooks/useChatSubmit.ts`                 | Message submission logic                                      |
| `src/features/agent/hooks/useAgentModels.ts`                | Model selection & provider switching                          |
| `src/features/agent/hooks/useInputToken.ts` (6 KB)          | Token estimation for input tracking                           |
| `src/features/agent/hooks/useAgentFileAttachment.ts` (8 KB) | File attachment handling                                      |
| `src/features/agent/hooks/useScopedSkills.ts`               | Per-session skill scoping                                     |

---

## 2. MCP Integration Layer

Full Model Context Protocol support — both built-in and external servers.

### 2a. Built-in MCP Servers (Rust implementations)

| Server             | Directory                                   | Key Tools / Capabilities                                                                                                                                                      |
| ------------------ | ------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Workspace**      | `src-tauri/src/mcp/builtin/workspace/`      | File read/write/list/search, shell commands (runShell/runInPersistentShell), process management, directory operations, file export, code execution, persistent shell sessions |
| **Planning**       | `src-tauri/src/mcp/builtin/planning/`       | Goal management, todo CRUD, scratchpad (working notes)                                                                                                                        |
| **Browser**        | `src-tauri/src/mcp/builtin/browser/`        | Headless browser automation: navigate, screenshot, execute JS, extract content, session management                                                                            |
| **Knowledge**      | `src-tauri/src/mcp/builtin/knowledge/`      | Global knowledge base: create, read, update, delete, search, chunking, relationship management                                                                                |
| **Playbook**       | `src-tauri/src/mcp/builtin/playbook/`       | Playbook CRUD: create, execute, bookmark, search reusable workflows                                                                                                           |
| **Agent**          | `src-tauri/src/mcp/builtin/agent/`          | Session listing, deletion, bookmarking, viewing metadata                                                                                                                      |
| **Session API**    | `src-tauri/src/mcp/builtin/session_api/`    | Session-level API tools for the agent to introspect its own state                                                                                                             |
| **Skills**         | `src-tauri/src/mcp/builtin/skills/`         | Skill scanning, content retrieval, directory listing                                                                                                                          |
| **Media**          | `src-tauri/src/mcp/builtin/media/`          | Image/text generation via HuggingFace, speech-to-text, TTS, embeddings, text classification                                                                                   |
| **Attachments**    | `src-tauri/src/mcp/builtin/attachments/`    | Session-scoped file attachment management                                                                                                                                     |
| **Bootstrap**      | `src-tauri/src/mcp/builtin/bootstrap/`      | System initialization, app info, health checks                                                                                                                                |
| **Tool**           | `src-tauri/src/mcp/builtin/tool/`           | Self-reflection tools: list available tools, describe capabilities                                                                                                            |
| **UI**             | `src-tauri/src/mcp/builtin/ui/`             | Rich UI interaction tools                                                                                                                                                     |
| **Error Guidance** | `src-tauri/src/mcp/builtin/error_guidance/` | Error categorization and guided recovery                                                                                                                                      |
| **History**        | `src-tauri/src/mcp/builtin/history/`        | Session history access via MCP                                                                                                                                                |

### 2b. External MCP Server Management

| Component                | File                                                                  | Description                                                 |
| ------------------------ | --------------------------------------------------------------------- | ----------------------------------------------------------- |
| **Server Config CRUD**   | `src-tauri/src/commands/mcp_server_config_commands.rs`                | Create/update/delete/list MCP server configurations         |
| **Transport Support**    | `src-tauri/src/mcp/types.rs` — `TransportConfig`                      | stdio transport (local) and HTTP transport (remote)         |
| **OAuth 2.1 Auth**       | `src-tauri/src/mcp/oauth.rs` (10 KB)                                  | OAuth token management, discovery, PKCE support             |
| **Keychain Integration** | `src-tauri/src/mcp/keychain.rs` (4 KB)                                | Secure credential storage via OS keychain                   |
| **Session Isolation**    | `src-tauri/src/mcp/session_isolation/`, `session_isolation_config.rs` | Per-session MCP server instances                            |
| **Service Proxy**        | `src-tauri/src/mcp/service_proxy/`, `service_proxy_manager/`          | Unified routing: builtin vs external, per-session isolation |
| **Presets**              | `src-tauri/src/mcp/presets.rs` (4 KB)                                 | Pre-configured server templates                             |
| **Error Normalization**  | `src-tauri/src/mcp/error_normalization.rs` (11 KB)                    | Standardizes errors across protocol boundaries              |
| **Schema**               | `src-tauri/src/mcp/schema.rs` (8 KB)                                  | JSON Schema validation for tool inputs/outputs              |
| **Probe Server**         | `probe_mcp_server`                                                    | Health check and capability discovery                       |
| **Validate Schema**      | `validate_tool_schema`                                                | Tool parameter schema validation                            |

---

## 3. Assistant (Agent Config) Management

| Component                 | File                                             | Description                                                      |
| ------------------------- | ------------------------------------------------ | ---------------------------------------------------------------- |
| **Entity**                | `src-tauri/src/entity/assistant.rs`              | Assistant config: name, system prompt, temperature, model, tools |
| **Service**               | `src-tauri/src/services/assistant_service.rs`    | CRUD operations                                                  |
| **Frontend**              | `src/features/assistant/`                        | List, Card, Editor pages                                         |
| **Built-in Tools Editor** | `src/features/assistant/BuiltInToolsEditor.tsx`  | Select which builtin tools each assistant gets                   |
| **Skills Editor**         | `src/features/assistant/SkillsEditor.tsx`        | Per-assistant skill assignment                                   |
| **Local Services Editor** | `src/features/assistant/LocalServicesEditor.tsx` | External MCP server assignment per assistant                     |
| **Batch Operations**      | `batch_upsert_assistants`                        | Bulk create/update assistants                                    |
| **Search**                | `search_assistants`                              | Full-text search across assistant configs                        |

---

## 4. Session & History Management

| Component              | File                                                  | Description                                                |
| ---------------------- | ----------------------------------------------------- | ---------------------------------------------------------- |
| **Entity**             | `src-tauri/src/entity/session.rs`                     | Session metadata: name, model, config, timestamps          |
| **Lifecycle**          | `src-tauri/src/agent/lifecycle/`                      | Create, delete, recover, pause, resume sessions            |
| **Frontend Agent**     | `src/features/agent/`                                 | AgentChatView, AgentDraftChatView (multi-agent chat)       |
| **History**            | `src/features/history/`                               | History panel, org view, lineage snapshots, org stat tiles |
| **Message Service**    | `src-tauri/src/services/message_service.rs` (12 KB)   | Paginated message retrieval, search, upsert                |
| **Message Entity**     | `src-tauri/src/entity/message.rs`                     | Conversation messages with role/content                    |
| **Message Index Meta** | `src-tauri/src/entity/message_index_meta.rs`          | Search index metadata                                      |
| **Compact Context**    | `src-tauri/src/entity/compact_context.rs`             | Stores context summaries for recovery                      |
| **Session Isolation**  | `src-tauri/src/session_isolation/`                    | Per-session workspace directories, state isolation         |
| **Session Cleanup**    | `src-tauri/src/services/session_cleanup_service.rs`   | Background cleanup of stale sessions                       |
| **Session Directory**  | `src-tauri/src/services/session_directory_service.rs` | Per-session file directory management                      |

### Org / Teamwork View

| File                                          | Description           |
| --------------------------------------------- | --------------------- |
| `src/features/history/Org.tsx`                | Org root view         |
| `src/features/history/OrgCard.tsx`            | Org card component    |
| `src/features/history/OrgLineageSnapshot.tsx` | Lineage visualization |
| `src/features/history/org-sessions.ts`        | Org session queries   |
| `src/features/history/org-status.ts`          | Org status indicators |

---

## 5. Knowledge Base System

| Component          | File                                                                                             | Description                                          |
| ------------------ | ------------------------------------------------------------------------------------------------ | ---------------------------------------------------- |
| **Entity**         | `src-tauri/src/entity/knowledge_entity.rs`, `knowledge_chunk_v2.rs`, `knowledge_relationship.rs` | Knowledge items with chunking and relationships      |
| **Service**        | `src-tauri/src/services/` (knowledge via builtin server)                                         | Global knowledge CRUD, search, chunk management      |
| **Frontend**       | `src/features/knowledge/KnowledgePage.tsx` (33 KB)                                               | Full knowledge management UI                         |
| **Tool Commands**  | `src-tauri/src/commands/knowledge_commands.rs`                                                   | list, get detail, delete global knowledge            |
| **Vector Support** | `chunk.rs`, `knowledge_chunk_v2.rs`                                                              | Chunk-based storage with potential embedding support |

---

## 6. Playbook System (Reusable Workflows)

| Component         | File                                                                           | Description                                                |
| ----------------- | ------------------------------------------------------------------------------ | ---------------------------------------------------------- |
| **Entity**        | `src-tauri/src/entity/playbook.rs`                                             | Playbook: goal, workflow steps, success criteria, bookmark |
| **Service**       | `src-tauri/src/services/playbook_service.rs`                                   | CRUD, search, bookmark toggle                              |
| **Frontend**      | `src/features/playbook/`                                                       | Playbook browser, selection, execution                     |
| **Tool Commands** | `src-tauri/src/commands/playbook_commands.rs`                                  | CRUD + bookmark operations                                 |
| **Steps**         | Workflow: description, action (tool + purpose), required data, output variable |
| **Integration**   | `src/features/agent/hooks/usePlaybookSearch.ts`                                | Playbook search within agent chat                          |

---

## 7. Scheduled Tasks

| Component             | File                                                       | Description                                               |
| --------------------- | ---------------------------------------------------------- | --------------------------------------------------------- |
| **Entity**            | `src-tauri/src/entity/scheduled_task.rs`                   | Task: cron expression, enabled flag, timezone, parameters |
| **Service**           | `src-tauri/src/services/scheduled_task_service.rs` (18 KB) | Cron-based task scheduling with timezone support          |
| **Background Worker** | `src-tauri/src/scheduled/`                                 | Cron-backed scheduled task background worker              |
| **Frontend**          | `src/features/scheduled-tasks/`                            | Task management UI                                        |
| **Tool Commands**     | `src-tauri/src/commands/scheduled_task_commands.rs`        | CRUD + toggle enable/disable                              |

---

## 8. Interactive Browser

| Component           | File                                                                                                                                                                        | Description                                     |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| **Server**          | `src-tauri/src/services/interactive_browser_server/`                                                                                                                        | Headless browser server with session management |
| **Browser Session** | `BrowserSession`, `InteractiveBrowserServer`                                                                                                                                | Session lifecycle, cleanup on app exit          |
| **Error Handling**  | `src-tauri/src/services/browser_error.rs`                                                                                                                                   | Browser error categorization and recovery       |
| **Content Store**   | `src-tauri/src/mcp/builtin/browser_content_store.rs`                                                                                                                        | Browser content caching                         |
| **Commands**        | `create_browser_session`, `close_browser_session`, `navigate_to_url`, `execute_script`, `browser_script_result`, `browser_page_loaded`, `navigate_back`, `navigate_forward` | Full browser automation API                     |

---

## 9. Workspace & File Operations

| Component               | File                                                      | Description                                      |
| ----------------------- | --------------------------------------------------------- | ------------------------------------------------ |
| **Secure File Manager** | `src-tauri/src/services/secure_file_manager.rs` (10 KB)   | Path validation, security checks, access control |
| **Workspace Service**   | `src-tauri/src/services/workspace_service.rs` (11 KB)     | File operations, directory management            |
| **File Export**         | `src-tauri/src/services/file_export_service.rs`           | ZIP export, media downloads                      |
| **Dropped Files**       | `src-tauri/src/services/dropped_file_service.rs` (19 KB)  | Drag-and-drop file handling                      |
| **Frontend Backend**    | `src/lib/backend/workspace.ts`, `file-operations.ts`      | TypeScript wrappers for workspace operations     |
| **Persistent Shell**    | `src-tauri/src/mcp/builtin/workspace/persistent_shell/`   | Stateful shell sessions with env persistence     |
| **Process Registry**    | `src-tauri/src/mcp/builtin/workspace/terminal_manager.rs` | Process lifecycle: start, poll, cancel, cleanup  |
| **Code Execution**      | `src-tauri/src/mcp/builtin/workspace/code_execution/`     | Jupyter-compatible code execution                |

---

## 10. Skills Management

| Component          | File                                                                                                                                                                          | Description                                           |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| **Skill Service**  | `src-tauri/src/services/skill_service/`                                                                                                                                       | Skill scanning, content loading, directory management |
| **Skill Commands** | `src-tauri/src/commands/skill_commands.rs`, `src-tauri/src/commands/skill_management.rs`                                                                                      | Scan, get content, import, install from GitHub        |
| **Frontend**       | `src/features/skills/`                                                                                                                                                        | Skills browser and management                         |
| **Tool Commands**  | `get_aggregated_skills`, `scan_skills_directory`, `get_skill_content`, `list_workspace_file_paths`                                                                            |                                                       |
| **Management**     | `copy_global_to_assistant`, `delete_assistant_skill`, `import_assistant_skills`, `import_user_skills`, `install_github_skills`, `reset_user_skills`, `reset_assistant_skills` |                                                       |

---

## 11. Settings & Configuration

| Component    | File                                              | Description                                                           |
| ------------ | ------------------------------------------------- | --------------------------------------------------------------------- |
| **Entity**   | `src-tauri/src/entity/settings.rs`                | Key-value settings store                                              |
| **Service**  | `src-tauri/src/services/` (settings via commands) | CRUD for application settings                                         |
| **Frontend** | `src/features/settings/`                          | Settings UI                                                           |
| **Config**   | `src-tauri/src/config.rs` (9 KB)                  | Application-level configuration                                       |
| **State**    | `src-tauri/src/state.rs` (17 KB)                  | Global app state: repositories, DB connections, service proxy manager |

---

## 12. Database Layer (SQLite + SeaORM)

| Component            | Description                                                                    |
| -------------------- | ------------------------------------------------------------------------------ |
| **Entities**         | `src-tauri/src/entity/` — 20 entity files covering all domain objects          |
| **Repositories**     | `src-tauri/src/repositories/` — SeaORM data access layer                       |
| **Migration**        | `migration` crate — Schema versioning and upgrades                             |
| **Schema Validator** | `src-tauri/src/db_schema_validator.rs` (10 KB) — Database integrity validation |
| **SQLite URL**       | Configurable SQLite connection URL via state management                        |

### Entity Inventory

| Entity                                 | File                                                                                    |
| -------------------------------------- | --------------------------------------------------------------------------------------- |
| `assistant`                            | `assistant.rs`                                                                          |
| `session`                              | `session.rs`                                                                            |
| `message`                              | `message.rs`                                                                            |
| `knowledge` + `chunk` + `relationship` | `knowledge_entity.rs`, `chunk.rs`, `knowledge_relationship.rs`, `knowledge_chunk_v2.rs` |
| `planning` (goal/todo/scratchpad)      | `planning_goal.rs`, `planning_todo.rs`, `planning_scratchpad.rs`                        |
| `playbook`                             | `playbook.rs`                                                                           |
| `scheduled_task`                       | `scheduled_task.rs`                                                                     |
| `mcp_server`                           | `mcp_server.rs`                                                                         |
| `settings`                             | `settings.rs`                                                                           |
| `compact_context`                      | `compact_context.rs`                                                                    |
| `content`                              | `content.rs`                                                                            |
| `store`                                | `store.rs`                                                                              |
| `message_index_meta`                   | `message_index_meta.rs`                                                                 |

---

## 13. Download & File Operations

| Feature            | Command                                       | Description                    |
| ------------------ | --------------------------------------------- | ------------------------------ |
| Media Download     | `download_media_file`                         | Download media files from URLs |
| Workspace Download | `download_workspace_file`                     | Download from workspace        |
| ZIP Export         | `export_and_download_zip`                     | Export multiple files as ZIP   |
| File Write         | `write_file`, `workspace_write_file`          | Save files to disk/workspace   |
| Dropped Files      | `register_dropped_files`, `read_dropped_file` | Handle drag-and-drop           |

---

## 14. Search

| Component            | Description                                                  |
| -------------------- | ------------------------------------------------------------ |
| **Module**           | `src-tauri/src/search/`                                      |
| **Message Search**   | `messages_search` — Full-text search across session messages |
| **Assistant Search** | `search_assistants` — Search across assistant configs        |
| **Index Meta**       | `message_index_meta.rs` — Search index metadata              |

---

## 15. Logging System

| Feature            | Command                                                                                                 | Description                                     |
| ------------------ | ------------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| Structured Logging | `log_trace`, `log_debug`, `log_info`, `log_warn`, `log_error_from_frontend`                             | Multi-level logging                             |
| Batch Logging      | `log_batch`                                                                                             | Batch log entries                               |
| Log Management     | `backup_current_log`, `clear_current_log`, `list_log_files`, `get_launch_log_level`, `get_app_logs_dir` | Log file operations                             |
| Logger Service     | `src-tauri/src/services/log_service.rs`                                                                 | Log file management                             |
| Custom Logger      | `src-tauri/src/logger.rs`                                                                               | File-based logger with Tauri plugin integration |

---

## 16. URL & System Integration

| Feature     | Command                                | Description                           |
| ----------- | -------------------------------------- | ------------------------------------- |
| Open URL    | `open_external_url`                    | Open URLs in system browser           |
| Explorer    | `open_workspace_in_explorer`           | Open workspace in OS file explorer    |
| Terminal    | `open_workspace_in_terminal`           | Open workspace in system terminal     |
| File Open   | `open_workspace_file_with_default_app` | Open file with default OS application |
| Restart App | `restart_app`                          | Hot-restart the Tauri application     |

---

## Architecture Summary

```
Frontend (React + TypeScript)
├── features/agent/          → Agent chat, session management, streaming
├── features/assistant/      → Assistant (agent config) CRUD
├── features/history/        → Session history, org view
├── features/knowledge/      → Global knowledge base
├── features/mcp/            → MCP type definitions
├── features/mcp-servers/    → External MCP server management
├── features/playbook/       → Reusable workflow editor
├── features/scheduled-tasks/ → Cron task management
├── features/settings/       → Application settings
├── features/skills/         → Skills browser & management
└── lib/backend/             → Tauri command wrappers (38 files)

Backend (Rust + Tauri)
├── agent/                   → Think-Act-Observe loop, session lifecycle, workflow
├── mcp/                     → Built-in servers, external MCP, session isolation
│   └── builtin/             → 15 built-in MCP servers
├── services/                → 18 service modules (browser, workspace, knowledge, etc.)
├── commands/                → Tauri command handlers (50+ commands)
├── entity/                  → 20 SeaORM entity definitions
├── repositories/            → Data access layer
├── session_isolation/       → Per-session state isolation
├── scheduled/               → Cron-based background worker
└── search/                  → Full-text search
```

**Total Commands Registered**: 100+ Tauri invoke handlers
**Builtin MCP Servers**: 15
**External MCP Support**: stdio + HTTP with OAuth 2.1
**Database**: SQLite via SeaORM with migration system
**Language**: Rust 100% — zero `any` types in TypeScript
