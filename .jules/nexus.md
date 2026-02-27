# Nexus Architecture Log

Format: `## YYYY-MM-DD - [Architecture] **Anti-Pattern:** [Spaghetti/Coupling] **Resolution:** [Design Pattern Applied]`

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
- **Result:** Command handlers in `agent_commands.rs` are now thin wrappers delegating to `AgentService`. The core logic is decoupled from Tauri `State` and is easier to test.
