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

## 2026-03-06 - Remaining Untyped Tauri commands

**Problem:** Several `invoke` calls for various Tauri backend commands (`agent_set_yolo_mode`, `restart_app`, `agent_create_session`, `agent_delete_session`, `agent_delete_session_only`, `agent_toggle_session_bookmark`, `agent_update_session_config`, `set_setting`, `delete_setting`, `update_settings`) were still missing explicit TypeScript generic types, bypassing `invoke`'s type safety.
**Action:**

- **Type Sync:** Applied strict generic return types (`safeInvoke<Type>`) across the remaining files in the codebase. Reused existing strong interfaces (`AgentResponse`, `AgentSessionMetadata`, `SettingDto`, `SettingDto[]`, and `<void>`).

## 2026-03-07 - Remaining Untyped Tauri commands II

**Problem:** Missed replacing raw `invoke` with `safeInvoke` for several files while adding type safety to generic calls.
**Action:**

- **Type Sync:** Replaced `invoke` from `@tauri-apps/api/core` with typed `safeInvoke` in context files (AgentChatContext, AgentSessionContext, AgentSessionListContext, SkillsContext), features (AgentDraftChatView, agent-backend, AgentChatStatusBar, GeneralTab), and hooks (useLLMListener).

## 2026-03-09 - Messages IPC Type Boundary

**Problem:** Untyped `safeInvoke` calls for message retrieval and search (`messages_get_page`, `messages_search`) using `Record<string, unknown>`, risking runtime errors and payload mismatches.

**Action:**

- **Type Sync:** Replaced `Record<string, unknown>` with strictly typed `RustMessage` and `RustSearchResult` in `src/lib/backend/messages.ts`, ensuring compile-time safety and 1:1 Rust-TS synchronization for message pagination and search.

## 2026-03-11 - Agent Error IPC Boundary

**Problem:** Direct unstructured `safeInvoke` for `agent_handle_llm_error` in `useLLMListener.ts` led to scattered logic and lack of type safety.

**Action:**

- **IPC Fix:** Extracted raw IPC call to a strictly typed `handleLLMError` wrapper in `src/lib/backend/agent-commands.ts`.
- **Refactor:** Updated `useLLMListener.ts` and associated tests to use the new type-safe boundary.

## 2026-03-12 - IPC JSON Payload Serialization Optimization

**Problem:** Redundant JSON parsing overhead on the bridge: `assistants.ts` and `mcp-server-config.ts` were stringifying JSON configurations, parsing them back to objects to send via `invoke`, while the Rust backend (`Value`) was parsing and stringifying them again to interact with the database.

**Action:**

- **Bridge Payload Optimization:** Changed frontend TS and Tauri Rust boundaries to pass the raw stringified configurations (`String` in Rust, `string` in TS) instead of `any` / `Value`.
- **Refactor:** Eliminated `JSON.parse(params.config)` from `safeInvoke` calls for `createAssistant`, `updateAssistant`, `upsertAssistants`, `createMCPServer`, and `updateMCPServer`, significantly reducing JSON parsing overhead.

## 2026-03-12 - Core Logger IPC Safety Optimization

**Problem:** `src/lib/logger.ts` was using untyped/raw `invoke` calls from `@tauri-apps/api/core` for `log_batch`, `get_app_logs_dir`, `backup_current_log`, `clear_current_log`, and `list_log_files` to avoid an infinite recursion loop where `safeInvoke` logs its own execution.

**Action:**

- **IPC Fix:** Updated `src/lib/backend/core.ts` `safeInvoke` to bypass logging for specific commands (`log_batch`, `get_app_logs_dir`, etc.).
- **Optimized:** Replaced all raw `invoke` calls in `src/lib/logger.ts` with `safeInvoke` to strictly enforce type boundaries and centralized error handling across the entire IPC bridge.
