# Resolve's Journal - Debt Repayment Log

## 2025-05-27 - [src/lib/backend/session-crud.ts] **Debt Cleared:** `// TODO: extract from assistants` **Solution:** Implemented logic to aggregate unique `mcpServerIds` from all assistants in the session and pass them to the backend `AgentConfig` as `mcpServerIds` (renamed from `mcpServers`). This ensures the backend correctly receives and persists the enabled MCP servers for the session.

## 2026-02-07 - src-tauri/src/agent/session_manager.rs

**Debt Cleared:** `// For now, to keep it simple and compile-safe, I will reference tools::handle_tool_result and add a TODO or duplicate the continuation logic if needed.`
**Solution:** Extracted `handle_tool_result_and_continue` from `llm.rs` into a shared function `continue_workflow_after_tool` in `workflow.rs` and updated both `llm.rs` and `session_manager.rs` to use it, eliminating code duplication and the TODO.

## 2025-02-23 - [src-tauri/src/lib.rs] **Debt Cleared:** `// TODO: Make port configurable via settings` **Solution:** Added `httpServerPort` to `SystemSettings` in both frontend and backend (with default `3030`). Updated `server::init` to use the configured port and exposed the setting in the Advanced Settings UI.
