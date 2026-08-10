# Harness Handbook — LibrAgent Behavior-Centric Code Map

> A behavior-centric representation linking source code to system behaviors.
> Organized by execution stages rather than file paths.

---

## Table of Contents

- [L1: System Overview](#l1-system-overview)
- [L2: Component Overview](#l2-component-overview)
- [L3: Unit Deep Dive](#l3-unit-deep-dive)
  - [2.1 Application Startup & Initialization](#21-application-startup--initialization)
  - [2.2 Session Lifecycle Management](#22-session-lifecycle-management)
  - [2.3 Workflow Orchestration](#23-workflow-orchestration)
  - [2.4 LLM Completion Pipeline](#24-llm-completion-pipeline)
  - [2.5 Tool Execution Engine](#25-tool-execution-engine)
  - [2.6 Frontend Reactivity Layer](#26-frontend-reactivity-layer)
  - [2.7 Data Persistence Layer](#27-data-persistence-layer)
  - [2.8 MCP Infrastructure](#28-mcp-infrastructure)
  - [2.9 Scheduled Task System](#29-scheduled-task-system)
  - [2.10 Browser Automation](#210-browser-automation)
  - [2.11 Configuration & Settings](#211-configuration--settings)
- [State-Register View](#state-register-view)

---

## L1: System Overview

### Architectural Pipeline

LibrAgent is a Tauri 2.x desktop application with a Rust backend and React/TypeScript frontend. The architecture follows a **Rust-orchestrated, frontend-reactive** pattern:

```
┌─────────────────────────────────────────────────────────────────────┐
│                        FRONTEND (React/TS)                          │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────────────┐   │
│  │  Routes      │  │ Context      │  │ Feature Components       │   │
│  │ (13 pages)   │→ │ Providers    │→ │ (Agent, Assistant, etc.) │   │
│  └─────────────┘  └──────┬───────┘  └──────────────────────────┘   │
│                          │ events/state                            │
│  ┌───────────────────────┴──────────────────────────────────────┐   │
│  │  Tauri Command Wrappers (lib/backend/*.ts) via safeInvoke()  │   │
│  └──────────────────────────┬───────────────────────────────────┘   │
│                              │ IPC                                   │
└──────────────────────────────┼───────────────────────────────────────┘
                               │
┌──────────────────────────────┼───────────────────────────────────────┐
│                        BACKEND (Rust/Tauri)                          │
│  ┌─────────────────────┐     ┌──────────────────────────────────┐   │
│  │ Tauri Command       │     │ AgentSessionManager (facade)     │   │
│  │ Handlers (~80 cmds) │────→│  ├─ lifecycle/   (create/kill)   │   │
│  │ (commands/*.rs)     │     │  ├─ workflow/    (start/finish)  │   │
│  └─────────┬───────────┘     │  ├─ llm/         (completion)    │   │
│            │                 │  └─ tools/       (dispatch)      │   │
│            │                 └──────────────────────────────────┘   │
│            │                              │                         │
│            │                 ┌────────────┴────────────┐           │
│            │                 │ MCPServiceProxy          │           │
│            │                 │  ├─ builtin/ (17 servers)│           │
│            │                 │  └─ external (stdio/HTTP)│           │
│            │                 └────────────┬────────────┘           │
│            │                              │                         │
│            │                 ┌────────────┴────────────┐           │
│            │                 │ Repository Layer         │           │
│            │                 │ (13 SeaORM repositories) │           │
│            │                 └────────────┬────────────┘           │
│            │                              │                         │
│            │                 ┌────────────┴────────────┐           │
│            │                 │ SQLite Database          │           │
│            │                 └─────────────────────────┘           │
└─────────────────────────────────────────────────────────────────────┘
```

### High-Level Execution Stages

| #   | Stage                      | Description                                                                                                   |
| --- | -------------------------- | ------------------------------------------------------------------------------------------------------------- |
| 1   | **Startup & Init**         | `main.rs` → env loading → SQLite setup → Tauri builder → `app_setup()` → global state                         |
| 2   | **Session Management**     | Create/resume/pause/terminate/delete agent sessions with workspace isolation                                  |
| 3   | **Workflow Orchestration** | `start_workflow` → LLM completion → tool execution loop → `finish_workflow`                                   |
| 4   | **LLM Completion**         | Prompt building → context selection → compaction decision → API request → response handling → circuit breaker |
| 5   | **Tool Execution**         | `MCPServiceProxy` routes to builtin servers or external MCP (stdio/HTTP)                                      |
| 6   | **UI Reactivity**          | Backend events → `AgentSessionContext` → React state updates → component re-render                            |
| 7   | **Data Persistence**       | All state persisted via SeaORM repositories to SQLite                                                         |
| 8   | **MCP Infrastructure**     | Session-isolated stdio/HTTP managers, service proxy manager, OAuth                                            |
| 9   | **Scheduled Tasks**        | Cron-backed background worker for recurring tasks                                                             |
| 10  | **Browser Automation**     | Interactive browser server with session management                                                            |
| 11  | **Configuration**          | Settings, assistant configs, MCP server configs, skills                                                       |

---

## L2: Component Overview

Each stage below is broken into its constituent components with behavioral descriptions.

### 2.1 Application Startup & Initialization

**Behavior:** Loads environment, initializes SQLite, registers all Tauri commands, creates global state singletons, starts background workers.

| Component         | Responsibility                                                                           |
| ----------------- | ---------------------------------------------------------------------------------------- |
| `main.rs`         | Entry point — env loading, SQLite path setup, sidecar detection                          |
| `lifecycle/`      | App lifecycle — `setup_app()`, `run_with_sqlite_sync()`, concurrency gate, session bus   |
| `state.rs`        | Global singletons — 13 repositories, MCPServiceProxyManager, SessionBus, ConcurrencyGate |
| `commands/mod.rs` | Tauri command registration — ~80 handlers grouped by domain                              |
| `scheduled/`      | Cron-backed scheduled task background worker                                             |

### 2.2 Session Lifecycle Management

**Behavior:** Creates, resumes, pauses, terminates, and deletes agent sessions with workspace isolation and Docker provisioning support.

| Component                       | Responsibility                                         |
| ------------------------------- | ------------------------------------------------------ |
| `agent/lifecycle/creation.rs`   | Session creation with workspace/Docker config          |
| `agent/lifecycle/deletion.rs`   | Session deletion with cleanup                          |
| `agent/lifecycle/recovery.rs`   | Session recovery from persisted state                  |
| `agent/lifecycle/cache.rs`      | Message cache initialization and synchronization       |
| `agent/lifecycle/management.rs` | Status transitions, queries, bookmark management       |
| `session/manager.rs`            | Workspace isolation manager                            |
| `session_isolation/`            | Platform-specific path mapping (Windows, macOS, Linux) |

### 2.3 Workflow Orchestration

**Behavior:** Manages the Think-Act-Observe loop — starts workflows, handles LLM responses, executes tools, manages pending events queue, finalizes sessions.

| Component                        | Responsibility                                                           |
| -------------------------------- | ------------------------------------------------------------------------ |
| `agent/workflow/start.rs`        | `start_workflow` — status checks, deduplication, queuing, event emission |
| `agent/workflow/tool.rs`         | Tool execution coordination                                              |
| `agent/workflow/finish.rs`       | `finish_workflow` — persist results, go idle, finalize errors            |
| `agent/workflow/cancel.rs`       | Soft/hard cancel with cancellation token management                      |
| `agent/workflow/pause_resume.rs` | Pause/resume workflow state                                              |
| `agent/session_manager.rs`       | `AgentSessionManager` facade — delegates to lifecycle/workflow/llm/tools |

### 2.4 LLM Completion Pipeline

**Behavior:** Builds prompts, selects context, decides compaction, sends API requests, handles responses, manages circuit breakers and streaming recovery.

| Component                                        | Responsibility                                                  |
| ------------------------------------------------ | --------------------------------------------------------------- |
| `agent/llm/prompt.rs`                            | System prompt construction with context providers               |
| `agent/llm/context_selector.rs`                  | Context selection and token budget management                   |
| `agent/llm/completion/request.rs`                | `request_llm_completion` — main completion orchestrator         |
| `agent/llm/completion/orchestration.rs`          | `request_llm_completion_with_recovery` — error recovery wrapper |
| `agent/llm/completion/compaction/trigger.rs`     | Compaction trigger decision                                     |
| `agent/llm/completion/compaction/selection.rs`   | Message selection for compaction                                |
| `agent/llm/completion/compaction/payload.rs`     | Compaction payload construction                                 |
| `agent/llm/completion/compaction/preparation.rs` | Compaction preparation and recovery                             |
| `agent/llm/response.rs`                          | Response handling — tool call extraction, persistence           |
| `agent/llm/tool_execution.rs`                    | Tool call dispatch from LLM response                            |
| `agent/llm/circuit_breaker.rs`                   | Circuit breaker for LLM errors                                  |
| `agent/llm/stream_recovery.rs`                   | Streaming response recovery                                     |
| `agent/llm/token_utils.rs`                       | Token counting and estimation                                   |

### 2.5 Tool Execution Engine

**Behavior:** Routes tool calls to builtin servers or external MCP servers, manages timeouts, normalizes errors, handles structured content.

| Component                                                        | Responsibility                                                      |
| ---------------------------------------------------------------- | ------------------------------------------------------------------- |
| `mcp/service_proxy/mod.rs`                                       | `MCPServiceProxy` — session-bound proxy, routes to builtin/external |
| `mcp/service_proxy/factory.rs`                                   | Builtin server instantiation                                        |
| `mcp/service_proxy/routing.rs`                                   | Tool name routing (builtin vs external)                             |
| `mcp/service_proxy_manager/`                                     | Session-specific proxy creation and management (11 files)           |
| `mcp/builtin/mod.rs`                                             | `BuiltinMCPServer` trait and `BuiltinServerRegistry`                |
| `mcp/builtin/{planning,knowledge,browser,scratchpad,skills,...}` | 17 builtin server implementations                                   |
| `mcp/session_isolation/stdio_manager/`                           | Stdio MCP server lifecycle (6 files)                                |
| `mcp/session_isolation/http_manager.rs`                          | HTTP MCP server session management                                  |
| `mcp/oauth.rs`                                                   | OAuth 2.1 flow management                                           |
| `mcp/error_normalization.rs`                                     | Error category normalization                                        |

### 2.6 Frontend Reactivity Layer

**Behavior:** React context providers consume backend events, manage local state, render UI components, handle streaming messages.

| Component                             | Responsibility                                          |
| ------------------------------------- | ------------------------------------------------------- |
| `app/main.tsx`                        | Vite entry — React root with providers, logger init     |
| `app/App.tsx`                         | Route definitions — 13 lazy-loaded routes               |
| `context/AgentSessionContext.tsx`     | `AgentSessionProvider` — session state, event listeners |
| `context/AgentSessionListContext.tsx` | Session list management                                 |
| `context/AgentChatContext.tsx`        | Chat message state and streaming                        |
| `context/LLMServiceContext.tsx`       | LLM service selection and configuration                 |
| `context/MCPServerContext.tsx`        | MCP server state                                        |
| `lib/backend/core.ts`                 | `safeInvoke()` — centralized Tauri command invocation   |
| `lib/backend/agent-commands.ts`       | Agent-specific command wrappers                         |
| `lib/ai-service/factory.ts`           | LLM provider factory                                    |
| `lib/ai-service/openai.ts`            | OpenAI-compatible API implementation                    |
| `lib/ai-service/anthropic.ts`         | Anthropic API implementation                            |
| `lib/ai-service/gemini.ts`            | Google Gemini API implementation                        |

### 2.7 Data Persistence Layer

**Behavior:** SeaORM-based repository pattern for SQLite — session CRUD, message storage, planning state, knowledge, settings.

| Component                                   | Responsibility                |
| ------------------------------------------- | ----------------------------- |
| `repositories/session_repository.rs`        | Session CRUD and metadata     |
| `repositories/message_repository.rs`        | Message storage and retrieval |
| `repositories/assistant_repository.rs`      | Assistant CRUD                |
| `repositories/planning_repository.rs`       | Planning state (goals, todos) |
| `repositories/knowledge_repository.rs`      | Knowledge base storage        |
| `repositories/mcp_server_repository.rs`     | MCP server config storage     |
| `repositories/playbook_repository.rs`       | Playbook CRUD                 |
| `repositories/scheduled_task_repository.rs` | Scheduled task storage        |
| `entity/`                                   | SeaORM entity definitions     |
| `migration/`                                | Database schema migrations    |

### 2.8 MCP Infrastructure

**Behavior:** Manages session-isolated MCP server connections, tool discovery, background caching, and service proxy lifecycle.

| Component                 | Responsibility                                       |
| ------------------------- | ---------------------------------------------------- |
| `mcp/server/mod.rs`       | MCP server lifecycle and tool management             |
| `mcp/server/lifecycle.rs` | Server start/stop lifecycle                          |
| `mcp/server/tools.rs`     | Tool registration and schema                         |
| `mcp/types.rs`            | MCP protocol types (MCPResponse, MCPTool, MCPResult) |
| `mcp/schema.rs`           | Tool schema validation                               |
| `mcp/presets.rs`          | MCP server presets                                   |
| `mcp/keychain.rs`         | Secure credential storage                            |

### 2.9 Scheduled Task System

**Behavior:** Cron-backed background worker that wakes assistants on schedule, manages task CRUD, handles execution modes.

| Component                                   | Responsibility                 |
| ------------------------------------------- | ------------------------------ |
| `scheduled/`                                | Cron worker background process |
| `commands/scheduled_task_commands.rs`       | Task CRUD Tauri commands       |
| `services/scheduled_task_service.rs`        | Task business logic            |
| `repositories/scheduled_task_repository.rs` | Task persistence               |

### 2.10 Browser Automation

**Behavior:** Manages interactive browser sessions via headless browser, provides browser tools for web interaction.

| Component                           | Responsibility                  |
| ----------------------------------- | ------------------------------- |
| `services/InteractiveBrowserServer` | Browser session management      |
| `commands/browser_commands.rs`      | Browser Tauri commands          |
| `browser_sidecar/`                  | Browser sidecar process         |
| `mcp/builtin/browser/`              | Browser MCP tool implementation |

### 2.11 Configuration & Settings

**Behavior:** Manages application settings, assistant configurations, model providers, and skill directories.

| Component                             | Responsibility            |
| ------------------------------------- | ------------------------- |
| `config.rs`                           | Application configuration |
| `commands/settings_commands.rs`       | Settings CRUD             |
| `commands/assistant_crud_commands.rs` | Assistant CRUD            |
| `commands/skill_commands.rs`          | Skill management          |
| `services/assistant_service.rs`       | Assistant business logic  |
| `services/assistant_init.rs`          | Assistant initialization  |

---

## L3: Unit Deep Dive

### 2.1 Application Startup & Initialization

#### `main.rs` — Application Entry Point

- **File:** `src-tauri/src/main.rs`
- **Key Functions:**
  - `main()` (L14): Entry point — checks for browser sidecar mode, loads `.env` files, sets SQLite DB path, calls `run_with_sqlite_sync()`
  - Browser sidecar detection (L13-20): If `--browser-sidecar` flag present, runs `browser_sidecar::run_sidecar_mode()`
  - Environment loading (L31-73): Development loads `.env.dev` → `.env`; Production loads `.env` from CWD or executable directory
  - SQLite path resolution: Uses `LIBRAGENT_DB_PATH` env, else debug → `libragent_v2.dev.db`, release → `libragent_v2.db` under the app data dir (so `tauri dev` never migrates the production DB by default)

#### `lifecycle/app_setup.rs` — Application Setup

- **File:** `src-tauri/src/lifecycle/app_setup.rs`
- **Behavior:** Called from `.setup()` hook in `lib.rs`. Initializes all global state singletons.
- **Key Actions:**
  1. Creates SQLite database connection
  2. Initializes all 13 repositories
  3. Creates `MCPServiceProxyManager`
  4. Initializes `SessionBus` (event bus)
  5. Initializes `ConcurrencyGate`
  6. Creates `InteractiveBrowserServer`
  7. Runs database migrations

#### `state.rs` — Global State Management

- **File:** `src-tauri/src/state.rs`
- **Key Globals (OnceLock):**
  - `MCP_SERVICE_PROXY_MANAGER` — session proxy manager (L26)
  - `SQLITE_DB_URL` — database URL (L29)
  - `DATABASE_CONNECTION` — SeaORM connection (L32)
  - `MESSAGE_REPOSITORY` — message store (L35)
  - `ATTACHMENTS_REPOSITORY` — attachment store (L38)
  - `SESSION_REPOSITORY` — session store (L41)
  - `SETTINGS_REPOSITORY` — settings store (L44)
  - `MCP_SERVER_REPOSITORY` — MCP server store (L47)
  - `ASSISTANT_REPOSITORY` — assistant store (L50)
  - `PLAYBOOK_REPOSITORY` — playbook store (L53)
  - `KNOWLEDGE_REPOSITORY` — knowledge store (L56)
  - `KNOWLEDGE_V2_REPOSITORY` — knowledge v2 store (L59)
  - `PLANNING_REPOSITORY` — planning store (L62)
  - `SCHEDULED_TASK_REPOSITORY` — task store (L65)
  - `COMPACT_CONTEXT_REPOSITORY` — compact context store (L68)
  - `APP_HANDLE` — Tauri AppHandle for events (L71)
  - `SESSION_BUS` — session event bus (L74)
  - `CONCURRENCY_GATE` — concurrency control (L77)
  - `ACTIVE_SESSIONS` — active sessions map (L82)
  - `CHANNEL_DISPATCH_AGENT` — agent manager for channel dispatch (L85)
- **Key Functions:** `set_*` / `get_*` accessors for each global, `init_app_handle()`, `start_startup_timer()`

#### `lib.rs` — Tauri Application Builder

- **File:** `src-tauri/src/lib.rs`
- **Key Functions:**
  - `run_with_sqlite_sync()` (L142): Wraps async initialization in Tokio runtime
  - `run()` (L156): Tauri builder — registers ~80 invoke handlers, `.setup()` → `lifecycle::app_setup::setup_app()`, `.build()`, exit hook for browser cleanup

---

### 2.2 Session Lifecycle Management

#### `AgentSessionManager` — Session Facade

- **File:** `src-tauri/src/agent/session_manager.rs`
- **Struct:** `AgentSessionManager` (L50-58) — facade delegating to lifecycle/workflow/llm/tools
- **Key Methods:**
  - `create_session()` (L137): Create new session with workspace isolation
  - `create_session_with_repo()` (L160): Create with specific repository and Docker config
  - `resume_session()` (L211): Resume a paused session
  - `get_session()` (L324): Fetch session metadata
  - `pause_workflow()` (L367): Pause active workflow
  - `resume_workflow()` (L378): Resume paused workflow
  - `terminate_session()` (L396): Hard terminate session
  - `cancel_workflow()` (L408): Soft cancel with cancellation token
  - `handle_tool_result()` (L476): Process tool execution result
  - `delete_session()` (L574): Delete session and all data
  - `delete_session_only()` (L586): Delete session data only (keep metadata)
  - `get_tools_for_session()` (L637): Enumerate available tools for session

#### `lifecycle/creation.rs` — Session Creation

- **File:** `src-tauri/src/agent/lifecycle/creation.rs`
- **Behavior:** Creates `AgentSession` in-memory, initializes message cache, creates workspace directory, provisions Docker if configured
- **Key Function:** `create_session()` — accepts `CreateSessionParams`, creates session with workspace isolation mode (Host, Docker, etc.)

#### `lifecycle/recovery.rs` — Session Recovery

- **File:** `src-tauri/src/agent/lifecycle/recovery.rs`
- **Behavior:** Recovers session from persisted state (messages, planning state, context) after restart

#### `lifecycle/cache.rs` — Message Cache

- **File:** `src-tauri/src/agent/lifecycle/cache.rs`
- **Behavior:** Initializes message cache for new sessions, synchronizes cache with database

#### `session/manager.rs` — Workspace Isolation

- **File:** `src-tauri/src/session/manager.rs`
- **Behavior:** Manages workspace directories per session, handles workspace overrides

#### `session_isolation/platforms/` — Platform Path Mapping

- **Files:** `windows.rs` (L1), `macos.rs` (L1), `linux.rs` (L1), `unix.rs` (L1)
- **Behavior:** Platform-specific path translation for cross-platform workspace isolation

---

### 2.3 Workflow Orchestration

#### `workflow/start.rs` — Workflow Startup

- **File:** `src-tauri/src/agent/workflow/start.rs`
- **Key Functions:**
  - `reset_session_execution_state()` (L13): Resets cancellation tokens, retry counters, compaction state
  - `start_workflow()` (L23): Main entry — ensures cache initialized, deduplicates message, checks status (Busy/Queued → queue), resets cancellation state, updates status to Queued, emits `WorkflowStarted` event, appends user message via `MessageService`

#### `workflow/finish.rs` — Workflow Finalization

- **File:** `src-tauri/src/agent/workflow/finish.rs`
- **Key Functions:**
  - `session_has_pending_events()` (L60): Check if session has queued messages
  - `continue_workflow_if_pending_events()` (L82): Drain pending events queue
  - `persist_terminal_assistant_sync()` (L115): Persist final assistant message
  - `settle_before_terminal_transition()` (L157): Pre-terminal state settlement
  - `settle_session_and_go_idle_with_dispatcher()` (L268): Settle and transition to Idle with event dispatch
  - `settle_session_and_go_idle()` (L343): Settle and transition to Idle
  - `settle_session_and_finalize_error_with_dispatcher()` (L370): Settle and transition to Error with dispatch
  - `settle_session_and_finalize_error()` (L410): Settle and transition to Error

#### `workflow/cancel.rs` — Workflow Cancellation

- **File:** `src-tauri/src/agent/workflow/cancel.rs`
- **Behavior:** Soft cancel (sets cancellation token, blocks stale responses) and hard cancel (terminates workflow)

#### `workflow/tool.rs` — Tool Execution Coordination

- **File:** `src-tauri/src/agent/workflow/tool.rs`
- **Behavior:** Coordinates tool call extraction from LLM response, dispatches to `MCPServiceProxy`, handles results

#### `workflow/pause_resume.rs` — Pause/Resume

- **File:** `src-tauri/src/agent/workflow/pause_resume.rs`
- **Behavior:** Pauses workflow (saves state, stops loop), resumes workflow (restores state, continues loop)

---

### 2.4 LLM Completion Pipeline

#### `completion/request.rs` — Main Completion Request

- **File:** `src-tauri/src/agent/llm/completion/request.rs`
- **Key Functions:**
  - `request_llm_completion()` (L43 of `request/orchestration.rs`): Main completion orchestrator — builds request, sends to LLM, handles response
  - `build_compact_summary_message()` (L18 of `request/compact.rs`): Build compaction summary message
  - `normalize_request_messages()` (L5 of `request/formatting.rs`): Normalize message list
  - `merge_consecutive_user_messages()` (L55 of `request/formatting.rs`): Merge consecutive user messages

#### `completion/orchestration.rs` — Recovery Wrapper

- **File:** `src-tauri/src/agent/llm/completion/orchestration.rs`
- **Key Function:**
  - `request_llm_completion_with_recovery()` (L9): Wraps `request_llm_completion()` with error recovery — on error, calls `handle_llm_error_with_outcome()` → `completion_result_from_error_handling_outcome()`

#### `completion/compaction/trigger.rs` — Compaction Trigger

- **File:** `src-tauri/src/agent/llm/completion/compaction/trigger.rs`
- **Key Functions:**
  - `trigger_preflight_compaction_for_session()` (L509): Trigger compaction before LLM request
  - `trigger_manual_compaction_for_session()` (L536): Trigger manual compaction

#### `completion/compaction/selection.rs` — Compaction Selection

- **File:** `src-tauri/src/agent/llm/completion/compaction/selection.rs`
- **Key Functions:**
  - `preview_preflight_compaction_selection()` (L84): Preview which messages would be compacted
  - `derive_tail_recompaction_recovery_plan()` (L119): Plan for tail recompaction recovery
  - `estimate_post_compact_resume_tokens()` (L211): Estimate tokens after compaction
  - `select_resume_fit_compaction_split()` (L431): Select split point for resume-fit compaction

#### `completion/compaction/payload.rs` — Compaction Payload

- **File:** `src-tauri/src/agent/llm/completion/compaction/payload.rs`
- **Key Functions:**
  - `apply_compaction_retry_budget()` (L53): Apply token budget for retry attempts
  - `fit_compaction_request_messages_to_limit()` (L252): Fit messages within token limit
  - `build_overflow_recovery_compaction_messages()` (L483): Build messages for overflow recovery

#### `response.rs` — Response Handling

- **File:** `src-tauri/src/agent/llm/response.rs`
- **Key Functions:**
  - `completion_result_from_error_handling_outcome()` (L26): Convert error handling outcome to Result
  - `persist_assistant_message_to_db()` (L45): Save assistant message to database
  - `cache_assistant_message()` (L59): Add assistant message to session cache (sliding window)
  - `extract_prompt_tokens()` (L90): Extract prompt token count from message
  - `persist_prompt_token_checkpoint()` (L101): Save prompt token checkpoint for compaction

#### `tool_execution.rs` — Tool Execution from Response

- **File:** `src-tauri/src/agent/llm/tool_execution.rs`
- **Behavior:** Extracts tool calls from assistant message, dispatches each to `MCPServiceProxy.call_tool()`, collects results

#### `circuit_breaker.rs` — Circuit Breaker

- **File:** `src-tauri/src/agent/llm/circuit_breaker.rs`
- **Behavior:** Detects repeated failures, stops LLM loops, triggers recovery or error finalization

#### `stream_recovery.rs` — Streaming Recovery

- **File:** `src-tauri/src/agent/llm/stream_recovery.rs`
- **Behavior:** Handles streaming response interruptions, recovers partial responses

#### `token_utils.rs` — Token Utilities

- **File:** `src-tauri/src/agent/llm/token_utils.rs`
- **Behavior:** Token counting, estimation, and budget management

---

### 2.5 Tool Execution Engine

#### `MCPServiceProxy` — Session-Bound Tool Proxy

- **File:** `src-tauri/src/mcp/service_proxy/mod.rs`
- **Struct:** `MCPServiceProxy` (L31-55) — session-bound, holds builtin_servers + stdio/HTTP caches
- **Key Methods:**
  - `builder()` (L92): Builder pattern for creating proxy
  - `create()` (L108): Internal creation — instantiates builtin servers from tool_ids
  - `call_tool()` (L160+): Main tool call — routes to builtin or external, handles timeout

#### `service_proxy/factory.rs` — Builtin Server Factory

- **File:** `src-tauri/src/mcp/service_proxy/factory.rs`
- **Behavior:** Creates session-specific builtin server instances (planning, knowledge, etc.)

#### `service_proxy/routing.rs` — Tool Routing

- **File:** `src-tauri/src/mcp/service_proxy/routing.rs`
- **Behavior:** Routes tool names to builtin servers (by service prefix) or external MCP servers

#### `service_proxy_manager/` — Proxy Manager

- **Directory:** `src-tauri/src/mcp/service_proxy_manager/`
- **Key Files:**
  - `creation.rs` (L1): Session-specific proxy creation
  - `management.rs` (L1): Proxy lifecycle management
  - `background_discovery.rs` (L1): Background tool discovery
  - `caching.rs` (L1): Tool cache management
  - `lazy_proxy.rs` (L1): Lazy proxy initialization
  - `runtime_updates.rs` (L1): Runtime proxy updates

#### `builtin/mod.rs` — Builtin Server Registry

- **File:** `src-tauri/src/mcp/builtin/mod.rs`
- **Trait:** `BuiltinMCPServer` (L43-100) — interface for all builtin servers
  - `name()`, `description()`, `tools()`, `call_tool()`, `get_service_context()`, `has_active_state()`
- **Struct:** `BuiltinServerRegistry` — registry of all builtin server instances
- **Sub-servers (17):** `agent`, `attachments`, `browser`, `browser_content_store`, `error_guidance`, `history`, `knowledge`, `media`, `planning`, `playbook`, `scheduled_task`, `scratchpad`, `service_id`, `setup_wizard`, `skills`, `tool`, `ui`, `workspace`

#### `session_isolation/stdio_manager/` — Stdio Manager

- **Directory:** `src-tauri/src/mcp/session_isolation/stdio_manager/`
- **Key Files:**
  - `execution.rs` (L1): Stdio process execution
  - `lifecycle.rs` (L1): Process lifecycle management
  - `cleanup.rs` (L1): Process cleanup
  - `permissions.rs` (L1): Process permission handling

#### `session_isolation/http_manager.rs` — HTTP Manager

- **File:** `src-tauri/src/mcp/session_isolation/http_manager.rs`
- **Behavior:** Manages HTTP-based MCP server sessions with connection pooling

---

### 2.6 Frontend Reactivity Layer

#### `main.tsx` — Vite Entry Point

- **File:** `src/app/main.tsx`
- **Behavior:** Creates React root with provider hierarchy: `BrowserRouter` → `ThemeProvider` → `SettingsProvider` → `ErrorBoundary` → `App`
- **Key Actions:** Initializes Tauri logger (`attachConsole()`), initializes global logger (`logUtils.initialize()`)

#### `App.tsx` — Route Configuration

- **File:** `src/app/App.tsx`
- **Behavior:** Defines 13 lazy-loaded routes with route mount markers for performance tracking
- **Routes:** `/agent`, `/agent/draft`, `/agent/:sessionId`, `/assistants`, `/playbooks`, `/history`, `/history/search`, `/org`, `/settings`, `/settings/migration`, `/mcp-servers`, `/knowledge`, `/scheduled-tasks`
- **Key Function:** `StartupRouteMountMarker` (L55) — marks route mount time for performance metrics

#### `context/AgentSessionContext.tsx` — Session State Provider

- **File:** `src/context/AgentSessionContext.tsx`
- **Key Exports:**
  - `AgentSessionProvider()` (L27): Provider — subscribes to Tauri events, manages session state
  - `useAgentSessionState()` (L129): Hook — reads session state (status, phase, messages)
  - `useOptionalAgentSessionState()` (L139): Hook — optional session state access
  - `useAgentSessionActions()` (L145): Hook — session action dispatchers
  - `useAgentSession()` (L155): Combined hook — state + actions

#### `lib/backend/core.ts` — Safe Invocation

- **File:** `src/lib/backend/core.ts`
- **Key Function:** `safeInvoke()` — centralized Tauri command invocation with error handling and logging

#### `lib/ai-service/factory.ts` — LLM Provider Factory

- **File:** `src/lib/ai-service/factory.ts`
- **Behavior:** Creates LLM provider instances based on configured provider type (OpenAI, Anthropic, Gemini, etc.)

#### `lib/ai-service/openai.ts` — OpenAI Provider

- **File:** `src/lib/ai-service/openai.ts`
- **Behavior:** OpenAI-compatible API implementation — chat completions, streaming, tool calls

---

### 2.7 Data Persistence Layer

#### `session_repository.rs` — Session Repository

- **File:** `src-tauri/src/repositories/session_repository.rs`
- **Behavior:** Session CRUD operations — create, read, update, delete, list, search
- **Key Operations:** `insert()`, `get()`, `list()`, `update()`, `delete()`, `search()`

#### `message_repository.rs` — Message Repository

- **File:** `src-tauri/src/repositories/message_repository.rs`
- **Behavior:** Message storage with pagination, search, and session-scoped queries
- **Key Operations:** `insert()`, `get_page()`, `get_messages_before()`, `search()`, `delete()`, `upsert_many()`

#### `planning_repository.rs` — Planning Repository

- **File:** `src-tauri/src/repositories/planning_repository.rs`
- **Behavior:** Stores session planning state (goals, todos, tasks)

#### `knowledge_repository.rs` — Knowledge Repository

- **File:** `src-tauri/src/repositories/knowledge_repository.rs`
- **Behavior:** Global knowledge base storage and retrieval

---

### 2.8 MCP Infrastructure

#### `mcp/types.rs` — Protocol Types

- **File:** `src-tauri/src/mcp/types.rs`
- **Key Types:** `MCPResponse`, `MCPTool`, `MCPResult`, `MCPContent`, `ServiceContext`, `ContextVolatility`

#### `mcp/schema.rs` — Schema Validation

- **File:** `src-tauri/src/mcp/schema.rs`
- **Behavior:** Validates tool schemas against MCP protocol requirements

#### `mcp/oauth.rs` — OAuth 2.1

- **File:** `src-tauri/src/mcp/oauth.rs`
- **Behavior:** OAuth 2.1 authentication flow for external MCP servers

---

### 2.9 Scheduled Task System

#### `commands/scheduled_task_commands.rs` — Task Commands

- **File:** `src-tauri/src/commands/scheduled_task_commands.rs`
- **Key Functions:** `create_scheduled_task`, `list_scheduled_tasks`, `get_scheduled_task`, `update_scheduled_task`, `toggle_scheduled_task`, `delete_scheduled_task`, `list_session_scheduled_tasks`, `cancel_session_scheduled_task`
- **Agent-facing MCP tools:** `scheduled_task__createScheduledTask`, `scheduled_task__listScheduledTasks`, `scheduled_task__getScheduledTask`, `scheduled_task__updateScheduledTask`, `scheduled_task__toggleScheduledTask`, `scheduled_task__deleteScheduledTask`, `scheduled_task__scheduleCallback`

#### `services/scheduled_task_service.rs` — Task Service

- **File:** `src-tauri/src/services/scheduled_task_service.rs`
- **Behavior:** Scheduled task business logic — validation, execution mode handling, cron parsing

---

### 2.10 Browser Automation

#### `services/InteractiveBrowserServer` — Browser Server

- **File:** `src-tauri/src/services/InteractiveBrowserServer/`
- **Behavior:** Manages headless browser sessions, provides browser tools for web interaction

#### `commands/browser_commands.rs` — Browser Commands

- **File:** `src-tauri/src/commands/browser_commands.rs`
- **Key Functions:** `create_browser_session`, `close_browser_session`, `navigate_to_url`, `execute_script`, `navigate_back`, `navigate_forward`

---

### 2.11 Configuration & Settings

#### `config.rs` — Application Config

- **File:** `src-tauri/src/config.rs`
- **Behavior:** Application-wide configuration — API keys, model defaults, feature flags

#### `commands/settings_commands.rs` — Settings Commands

- **File:** `src-tauri/src/commands/settings_commands.rs`
- **Key Functions:** `set_setting`, `get_setting`, `list_settings`, `delete_setting`, `update_settings`

#### `commands/assistant_crud_commands.rs` — Assistant Commands

- **File:** `src-tauri/src/commands/assistant_crud_commands.rs`
- **Key Functions:** `create_assistant`, `update_assistant`, `delete_assistant`, `list_assistants`, `get_assistant`, `search_assistants`, `batch_upsert_assistants`

---

## State-Register View

### Key Shared States and Transitions

| State                                      | Location                              | Type                                         | Transitions                                                                         |
| ------------------------------------------ | ------------------------------------- | -------------------------------------------- | ----------------------------------------------------------------------------------- |
| `ACTIVE_SESSIONS`                          | `state.rs:L120`                       | `Arc<RwLock<HashMap<String, AgentSession>>>` | Created on session start → populated during lifecycle → cleared on delete           |
| `MCP_SERVICE_PROXY_MANAGER`                | `state.rs:L38`                        | `Arc<MCPServiceProxyManager>`                | Initialized in `setup_app()` → persists for app lifetime                            |
| `DATABASE_CONNECTION`                      | `state.rs:L44`                        | `DatabaseConnection`                         | Created in `setup_app()` → persists for app lifetime                                |
| `SESSION_BUS`                              | `state.rs:L114`                       | `SessionBus`                                 | Initialized in `setup_app()` → emits events to frontend                             |
| `CONCURRENCY_GATE`                         | `state.rs:L117`                       | `ConcurrencyGate`                            | Initialized in `setup_app()` → controls parallel workflow limits                    |
| `AgentSession.status`                      | `state.rs` (AgentSession)             | `SessionStatus`                              | Idle → Queued → Busy → Idle/Error/Paused                                            |
| `AgentSession.messages`                    | `state.rs` (AgentSession)             | `Arc<RwLock<Vec<Message>>>`                  | Appended on user message → extended on assistant response → cached (sliding window) |
| `AgentSession.cancellation_token`          | `state.rs` (AgentSession)             | `CancellationToken`                          | Created on workflow start → cancelled on cancel → reset on new start                |
| `AgentSession.compaction`                  | `state.rs` (AgentSession)             | `CompactionState`                            | None → Preparing → InProgress → Complete/Error                                      |
| `MCPServiceProxy.builtin_servers`          | `service_proxy/mod.rs:L46`            | `HashMap<String, Box<dyn BuiltinMCPServer>>` | Created on proxy creation → persists for session lifetime                           |
| `MCPServiceProxy.session_stdio_tool_cache` | `service_proxy/mod.rs:L50`            | `Arc<RwLock<HashMap<String, Vec<MCPTool>>>>` | Populated during eager tool discovery → updated on runtime changes                  |
| `Message.prompt_tokens`                    | `entity/message.rs`                   | `Option<i64>`                                | Set on LLM response → persisted to DB → read for compaction decisions               |
| `PlanningRepository.state`                 | `repositories/planning_repository.rs` | `PlanningState`                              | Created on session start → updated during workflow → persisted on completion        |

### Data Flow Across Stages

```
User Input (Frontend)
  │
  ▼
Tauri Command (commands/agent_commands.rs)
  │
  ▼
AgentSessionManager.send_message() → start_workflow()
  │
  ├─► Session Status: Idle → Queued
  ├─► Message appended to session.messages
  ├─► WorkflowStarted event emitted
  │
  ▼
request_llm_completion_with_recovery()
  │
  ├─► Context selection & token budget check
  ├─► Compaction decision (trigger_preflight_compaction_for_session)
  ├─► Prompt building (agent/llm/prompt.rs)
  ├─► LLM API request
  │
  ▼
LLM Response (response.rs)
  │
  ├─► Tool call extraction
  ├─► Assistant message persisted to DB
  ├─► Assistant message cached (sliding window)
  │
  ▼
Tool Execution (workflow/tool.rs → MCPServiceProxy.call_tool)
  │
  ├─► Builtin server call (e.g., planning__addTodo)
  └─► External MCP call (stdio/HTTP)
  │
  ▼
Tool Result Handling
  │
  ├─► Result added to conversation
  ├─► Status: Busy → Queued (if more messages) or Idle/Error
  └─► Pending events drained (continue_workflow_if_pending_events)
  │
  ▼
Frontend Event (TauriEventDispatcher)
  │
  ▼
AgentSessionContext (React) → UI Update
```

### State Transition Matrix

| Transition      | Trigger                                 | Guards                       | Side Effects                         |
| --------------- | --------------------------------------- | ---------------------------- | ------------------------------------ |
| Idle → Queued   | `start_workflow()`                      | Not Busy/Queued/Provisioning | Emit WorkflowStarted, append message |
| Queued → Busy   | LLM request sent                        | Cache initialized            | Emit WorkflowStarted                 |
| Busy → Idle     | `settle_session_and_go_idle()`          | No pending events            | Reset cancellation token             |
| Busy → Error    | `settle_session_and_finalize_error()`   | Error occurred               | Emit error event                     |
| Busy → Queued   | `continue_workflow_if_pending_events()` | Pending events exist         | Continue loop                        |
| Idle → Paused   | `pause_workflow()`                      | Not already paused           | Save state                           |
| Paused → Idle   | `resume_workflow()`                     | Not already resumed          | Restore state                        |
| Any → Cancelled | `cancel_workflow()`                     | —                            | Set cancellation token               |

---

_Generated from static analysis of libr-agent codebase._
_All L3 locators verified against source code._
