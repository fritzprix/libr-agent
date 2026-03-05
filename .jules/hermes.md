# Hermes's Journal - IPC Optimization Log

## 2025-05-18 - Skills IPC & Type Boundary

**Problem:** Duplicate `SkillMetadata` interfaces across 5 components and inefficient in-memory zip buffering in `download_global_skills`.

**Action:**

- **Type Sync:** Unified `SkillMetadata` in `src/types/skills.ts` to ensure 1:1 match with Rust struct, resolving type duplication and potential mismatches.
- **Memory Optimization:** Refactored `download_global_skills` in `src-tauri/src/commands/skill_management.rs` to stream the zip download directly to a temp file instead of buffering the entire archive in memory.

## 2025-05-24 - Agent Commands IPC & Type Boundary

**Problem:** Untyped `invoke` calls for critical agent commands (`agent_send_message`, `agent_create_session`) leading to potential payload mismatches and lack of type safety.

**Action:**

- **Type Sync:** Created `src/models/agent-ipc.ts` to strictly mirror Rust structs (`AgentConfig`, `CreateAgentSessionRequest`, `SendUserMessageRequest`, etc.) from `agent_commands.rs`.
- **Refactor:** Updated `AgentChatContext`, `AgentSessionContext`, and `AgentSessionListContext` to use strict TypeScript interfaces for all IPC calls, ensuring 1:1 type synchronization.

## 2025-05-25 - MCP & Skills IPC Type Boundary

**Problem:** Untyped `invoke` calls in `mcp-server-registry.ts` and `SkillsEditor.tsx` defaulting to `any`, risking runtime errors and payload mismatches.

**Action:**

- **Type Sync:** Enforced strict generic types for `list_available_builtin_server_definitions` (`invoke<BuiltinServerInfo[]>`) and skill management commands (`invoke<string>`), ensuring compile-time safety and 1:1 Rust-TS synchronization.

## 2026-02-24 - Settings IPC Batching

**Problem:** Chatty `updateSettings` calls in `rust-settings-service.ts` triggering multiple parallel `invoke("set_setting")` calls for a single user action, increasing IPC overhead.

**Action:**

- **Batching:** Implemented `update_settings` Tauri command accepting `HashMap<String, Value>` to process multiple setting updates in a single transaction.
- **Refactor:** Updated `src/lib/services/rust-settings-service.ts` to accumulate changes and send a single `invoke("update_settings")` payload.

## 2026-03-04 - Logger IPC Batching

**Problem:** Chatty logging system triggering individual `invoke` calls for every log message, flooding the IPC bridge during high-frequency events.

**Action:**

- **Batching:** Implemented `LogQueue` in `src/lib/logger.ts` to buffer log entries (limit 50 or 500ms timeout) and send them in a single `invoke("log_batch")` call.
- **Type Sync:** Defined `LogEntry` interface in TS and corresponding struct in Rust to ensure type safety.
- **Rust Command:** Added `log_batch` to `src-tauri/src/commands/log_commands.rs` to process batched log entries efficiently.

## 2026-02-28 - Generic Type Boundaries for Tauri commands

**Problem:** Several `invoke` calls for various Tauri backend commands (`agent_handle_llm_response`, `agent_get_available_tools`, `agent_call_builtin_tool`, `agent_create_session`, `agent_send_message`, `agent_update_session_config`, `get_assistant`, `log_batch`, `clear_current_log`, `delete_assistant`, `open_skills_directory_in_explorer`) were missing explicit TypeScript generic types, bypassing `invoke`'s type safety.
**Action:**

- **Type Sync:** Applied strict generic return types (`invoke<Type>`) across the codebase. Reused existing strong interfaces (`AgentResponse`, `MCPTool[]`, `MCPResult<T>`, `AgentSessionMetadata`, `AssistantDto`, and `<void>`).

## 2026-03-01 - All Tauri Commands IPC Error Handling

**Problem:** Direct `invoke` calls for Tauri commands had scattered error handling and inconsistent logging, making IPC failures hard to track and reason about across the frontend.

**Action:**

- **Centralized Wrapper:** Introduced `safeInvoke` in `src/lib/backend/core.ts` to wrap Tauri `invoke` with standardized error handling, logging, and typed responses.
- **Refactor:** Replaced explicit `invoke` usages across all Tauri command call sites with `safeInvoke`, ensuring consistent IPC behavior and observability.

## 2026-03-05 - File Manager IPC Safety

**Problem:** Potential infinite recursion if `TauriLogFileManager` uses the centralized `safeInvoke` wrapper, as `safeInvoke` triggers logging which in turn may trigger file management (e.g., startup backup).

**Action:**

- **IPC Safety:** Retained raw `invoke` from `@tauri-apps/api/core` for all methods in `TauriLogFileManager` (`get_app_logs_dir`, `backup_current_log`, `clear_current_log`, `list_log_files`) to avoid logger-IPC circular dependencies.
- **IPC Fix:** Likewise retained standard `invoke` for `log_batch` within `LogQueue` to prevent the same infinite recursion loop.
