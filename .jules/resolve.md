# Resolve's Journal - Debt Repayment Log

## 2025-05-27 - [src/lib/backend/session-crud.ts] **Debt Cleared:** `// TODO: extract from assistants` **Solution:** Implemented logic to aggregate unique `mcpServerIds` from all assistants in the session and pass them to the backend `AgentConfig` as `mcpServerIds` (renamed from `mcpServers`). This ensures the backend correctly receives and persists the enabled MCP servers for the session.
