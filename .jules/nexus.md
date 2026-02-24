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
