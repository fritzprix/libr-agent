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
