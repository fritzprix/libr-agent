## 2026-02-05 - src-tauri/src/mcp/builtin/workspace/code_execution/shell.rs

**Split:** `handlers.rs`, `persistent.rs`, `isolated.rs`, `async_exec.rs`
**Improvement:** Decoupled MCP request handling from core execution logic. Separated persistent shell state management from isolated process execution. Reduced file size from >1100 lines to focused <300 line modules.

## 2026-02-05 - src/context/LLMServiceContext.tsx

**Split:** `types.ts`, `stream-processor.ts`, `useLLMState.ts`, `useCompletionExecutor.ts`, `useLLMListener.ts`, `index.tsx`
**Improvement:** Decomposed the massive LLM service context. Extracted complex stream processing logic into a dedicated accumulator class. Separated state management, execution logic, and event listening into custom hooks. Reduced maximum file size from ~1046 lines to ~260 lines (executor).
