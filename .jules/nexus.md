# Nexus Architecture Log

Format: `## YYYY-MM-DD - [Architecture] **Anti-Pattern:** [Spaghetti/Coupling] **Resolution:** [Design Pattern Applied]`

## 2025-03-06 - [Architecture] **Anti-Pattern:** Fat Handler **Resolution:** Service Extraction

- **Context:** The `log_commands.rs`, `content_store_commands.rs`, and `file_commands.rs` modules contained significant filesystem I/O and DB business logic.
- **Action:** Extracted `LogService`, `ContentStoreService`, and enhanced `WorkspaceService` to handle domain logic independently of Tauri's IPC boundary.
- **Result:** Command files now cleanly act as thin adapters, delegating domain responsibilities to isolated services.

## 2024-05-23 - [Architecture] **Anti-Pattern:** Fat Handler **Resolution:** Service Extraction

- **Context:** The `workspace_commands.rs` module contained significant business logic for listing files, managing workspace overrides, and platform-specific terminal launching.
- **Action:** Extracted `WorkspaceService` to handle workspace logic and `utils::terminal` to handle platform-specific terminal operations.
- **Result:** `workspace_commands.rs` now delegates to these services, adhering to the Single Responsibility Principle.

## 2026-02-23 - [Architecture] **Anti-Pattern:** Fat Handler **Resolution:** Service Extraction

- **Context:** The `skill_commands.rs` and `skill_management.rs` modules contained significant business logic for skill resolution, file system operations, and frontmatter parsing, violating the Separation of Concerns principle.
- **Action:** Extracted `SkillService` (`src-tauri/src/services/skill_service.rs`) to encapsulate skill-related business logic, including metadata parsing, resolution, and management.
- **Result:** Command handlers are now thin wrappers delegating to `SkillService`, making the logic testable in isolation and reusable. Integration tests were moved to `src-tauri/tests/` to comply with the library configuration.

## 2026-02-23 - [Architecture] **Anti-Pattern:** Fat Handler & God Component **Resolution:** Service Extraction & Component Decomposition

- **Context:** `download_commands.rs` contained mixed business logic (file reading, zip creation) and UI interaction. `MCPServerDialog.tsx` was a "God Component" managing complex form state, validation, and rendering for multiple transport types.
- **Action:**
  - Extracted `FileExportService` (`src-tauri/src/services/file_export_service.rs`) to encapsulate file reading and ZIP creation logic.
  - Extracted `useMCPServerForm` hook to manage form state and validation logic.
  - Decomposed `MCPServerDialog` into `StdioForm`, `HttpForm`, and `EnvVarsForm` components.
- **Result:** Command handlers now delegate business logic to the service. The dialog component is now a pure presentational component that delegates logic to the hook and rendering to sub-components, improving maintainability and testability.

## 2026-03-03 - [Architecture] **Anti-Pattern:** Fat Handler & Infrastructure Leakage **Resolution:** Service Extraction & Model Segregation

- **Context:** `messages_commands.rs` contained `Message` struct definition (DTO), search index caching logic (`INDEX_CACHE`), and business logic for message deletion and search, mixing concerns.
- **Action:**
  - Extracted `Message` struct to `src-tauri/src/models/chat.rs` to decouple data shape from command handler.
  - Extracted `MessageService` (`src-tauri/src/services/message_service.rs`) to encapsulate business logic for message deletion (DB + cache) and search (index management).
- **Result:** `messages_commands.rs` is now a thin wrapper. The `Message` model is reusable across the application. Search logic is centralized and testable.

## 2026-02-27 - [Architecture] **Anti-Pattern:** Fat Handler **Resolution:** Service Extraction

- **Context:** `agent_commands.rs` contained significant business logic for deleting all sessions and performing a factory reset, violating the Separation of Concerns principle.
- **Action:**
  - Extracted `AgentService` (`src-tauri/src/services/agent_service.rs`) to encapsulate the domain logic for `clear_all_sessions` and `factory_reset`.
- **Result:** Command handlers in `agent_commands.rs` are now thin wrappers delegating to `AgentService`. The handlers are decoupled from Tauri `State`; the service still relies on global repository singletons, so full unit-test isolation requires a future dependency-injection refactor.

## 2026-03-03 - [Architecture] **Anti-Pattern:** Fat Handler **Resolution:** Service Extraction

- **Context:** `agent_commands.rs` still contained business logic such as session creation (repository selection based on ephemeral flags, workspace overrides), initial message orchestration, builtin tool calling, and service context fetching.
- **Action:** Extracted `create_session`, `create_session_with_initial_message`, `call_builtin_tool`, and `get_service_contexts` from `agent_commands.rs` into `AgentService`.
- **Result:** `agent_commands.rs` is strictly a framework boundary now, merely passing arguments to `AgentService`. Domain logic regarding agent lifecycle management has been centralized.

## 2026-03-03 - [Architecture] **Anti-Pattern:** Fat Handler **Resolution:** Service Extraction

- **Context:** `mcp_commands.rs` contained complex logic for probing MCP servers, including DB lookup, configuration parsing, server manager instantiation, tool probing, and clean-up.
- **Action:** Extracted the logic from `probe_mcp_server` into `McpServerService`.
- **Result:** `mcp_commands.rs` is strictly a framework boundary now, delegating to `McpServerService`.

## 2026-03-03 - [Architecture] **Anti-Pattern:** Fat Handler **Resolution:** Service Extraction

- **Context:** `playbook_commands.rs` contained business logic for playbooks, including resolving assistant ID from session and playbook sorting and listing.
- **Action:** Extracted the logic into `PlaybookService` and created `PlaybookDto`.
- **Result:** `playbook_commands.rs` delegates complex logic to `PlaybookService`.

## 2025-02-28 - [Architecture] **Anti-Pattern:** [Spaghetti/Coupling - InteractiveBrowserServer directly instantiating Tauri app handles] **Resolution:** [Dependency Inversion - Extracted BrowserEnvironment trait and Tauri adapter]

## 2025-03-01 - [Architecture] **Anti-Pattern:** [Spaghetti/Coupling in AgentSessionManager] **Resolution:** [Extracted Domain Logic into SessionCleanupService]

## 2025-03-05 - [Architecture] **Anti-Pattern:** Fat Command Handlers / Framework Coupling **Resolution:** Extracted Domain and DB logic into dedicated `AssistantService`, `McpServerService`, `PlaybookService`, and `ScheduledTaskService`, leaving Tauri commands as thin framework boundaries.

## 2026-03-08 - [Architecture] **Anti-Pattern:** Fat Handler **Resolution:** Service Extraction

- **Context:** The `session_commands.rs` module contained orchestration logic for tearing down auxiliary resources (search index, database metadata) alongside Tauri framework code.
- **Action:** Extracted the orchestration logic into `SessionCleanupService::remove_session_complete`.
- **Result:** `session_commands.rs` acts as a thin wrapper delegating to the domain service layer.

## 2026-03-08 - [Architecture] **Anti-Pattern:** Leaking Domain Logic **Resolution:** Encapsulation

- **Context:** `browser_commands.rs` duplicated URL validation checks (`url.trim().is_empty()`) before passing it down to `InteractiveBrowserServer`.
- **Action:** Removed the redundant URL validation from the command layer.
- **Result:** `InteractiveBrowserServer` handles its own parameter validation natively, allowing `browser_commands.rs` to remain a pure framework entry point.

## 2026-03-09 - [Architecture] **Anti-Pattern:** Fat Handler & Orchestrator **Resolution:** Service Extraction

- **Context:** `session_manager.rs` contained orchestration logic for cascading deletion of descendant workspaces and DB records rather than just delegating to a lifecycle module.
- **Action:** Extracted `delete_session` and `delete_session_only` functions to a new `src-tauri/src/agent/lifecycle/deletion.rs` module.
- **Result:** `AgentSessionManager` remains a thin facade over lifecycle management, properly delegating domain actions.

## 2026-03-09 - [Architecture] **Anti-Pattern:** Fat Handler & Mixed Domains **Resolution:** Service Extraction

- **Context:** The `workflow::start_workflow` and `AgentSessionManager::inject_messages` functions were orchestrating too many cross-cutting concerns (message caching, database persistence, and UI event emission) alongside their primary agent execution logic, violating Separation of Concerns.
- **Action:** Extracted message persistence, caching, queueing, and UI event emission into a dedicated `MessageService`. Extracted proxy creation into a focused `ensure_proxy_exists` helper.
- **Result:** `workflow.rs` and `session_manager.rs` now properly delegate to domain services, keeping the agent execution logic clean and decoupled from data storage concerns.

## 2025-03-12 - [Service Layer Dependency Inversion] **Anti-Pattern:** [God Module/Coupling/Hidden Dependencies] **Resolution:** [Applied Dependency Inversion Principle. Refactored AssistantService, McpServerService, PlaybookService, and ScheduledTaskService to accept their respective Repositories as arguments rather than instantiating them internally via global state fetchers. Tauri commands now act purely as controllers that fetch the dependency and pass it down, improving testability and separation of concerns.]

## 2025-03-13 - [Architecture] **Anti-Pattern:** React God Component (The Smart UI Anti-Pattern) **Resolution:** Custom Hook Pattern (Domain Logic Extraction)

- **Context:** `AgentDraftChatView.tsx` was a massive ~700-line monolithic component handling UI rendering, complex drag-and-drop file processing, backend IPC orchestration, binary file indexing, workspace directory writes, and agent session initialization all in one file.
- **Action:** Extracted all state management, file parsing, and Tauri backend coordination into a custom hook `useAgentDraftChat` (`src/features/agent/hooks/useAgentDraftChat.ts`).
- **Result:** The `AgentDraftChatView.tsx` component is now a pure presentation layer, adhering to Separation of Concerns and making the complex state logic testable and reusable.

## 2025-03-13 - [Architecture] **Anti-Pattern:** Infrastructure Leakage **Resolution:** Model Extraction (Dependency Inversion)

- **Context:** The `assistant_crud_commands.rs` file tightly coupled the `AssistantDto` and its `From<AssistantModel>` implementation with the Tauri command handlers.
- **Action:** Extracted `AssistantDto` into a new dedicated module `src-tauri/src/models/assistant.rs` alongside other DTOs.
- **Result:** The Tauri command module now simply imports the data structures, separating the framework boundary logic from the data shape definition and strictly adhering to Separation of Concerns.
