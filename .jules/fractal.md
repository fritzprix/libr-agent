## 2026-02-05 - src-tauri/src/mcp/builtin/workspace/code_execution/shell.rs

**Split:** `handlers.rs`, `persistent.rs`, `isolated.rs`, `async_exec.rs`
**Improvement:** Decoupled MCP request handling from core execution logic. Separated persistent shell state management from isolated process execution. Reduced file size from >1100 lines to focused <300 line modules.

## 2026-02-05 - src/features/agent/components/AgentMessageRenderer.tsx
**Split:** `markdown/` (CodeBlock, Components), `hooks/` (Logic), `content/` (Sub-Components)
**Result:** Reduced from 912 lines to 173 lines. Decoupled rendering logic from UI actions and content processing.
