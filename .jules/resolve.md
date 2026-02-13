# Resolve's Journal - Debt Repayment Log

## 2025-05-27 - [src/lib/backend/session-crud.ts] **Debt Cleared:** `// TODO: extract from assistants` **Solution:** Implemented logic to aggregate unique `mcpServerIds` from all assistants in the session and pass them to the backend `AgentConfig` as `mcpServerIds` (renamed from `mcpServers`). This ensures the backend correctly receives and persists the enabled MCP servers for the session.

## 2026-02-07 - src-tauri/src/agent/session_manager.rs

**Debt Cleared:** `// For now, to keep it simple and compile-safe, I will reference tools::handle_tool_result and add a TODO or duplicate the continuation logic if needed.`
**Solution:** Extracted `handle_tool_result_and_continue` from `llm.rs` into a shared function `continue_workflow_after_tool` in `workflow.rs` and updated both `llm.rs` and `session_manager.rs` to use it, eliminating code duplication and the TODO.

## 2026-02-09 - src-tauri/src/server/handlers.rs

**Debt Cleared:** `// TODO: might need to fetch assistant ID from config`
**Solution:** Extracted `assistant_id` from `agent_config` in `create_session` and `send_message` functions and populated the `Message` struct to ensure proper assistant tracking.

## 2026-02-10 - [src-tauri/src/mcp/service_proxy_manager/mod.rs] **Debt Cleared:** `/// - External HTTP tools -> shared HTTP manager (TODO: Phase 3)` **Solution:** Verified that `HttpSessionManager` correctly implements session-isolated HTTP connections with `Mcp-Session-Id` header injection. Removed the stale TODO comment as the implementation is complete.
