## 2026-02-05 - src-tauri/src/mcp/builtin/workspace/code_execution/shell.rs

**Split:** `handlers.rs`, `persistent.rs`, `isolated.rs`, `async_exec.rs`
**Improvement:** Decoupled MCP request handling from core execution logic. Separated persistent shell state management from isolated process execution. Reduced file size from >1100 lines to focused <300 line modules.

## 2026-02-07 - src-tauri/src/session_isolation.rs

**Split:** `types.rs`, `common.rs`, `platforms/mod.rs`, `platforms/windows.rs`, `platforms/linux.rs`, `platforms/macos.rs`, `platforms/unix.rs`, `mod.rs`
**Improvement:** Decoupled platform-specific isolation logic (Linux `unshare`, macOS `sandbox-exec`, Windows job objects) into dedicated modules. Separated types and common utilities. Reduced monolithic file size from ~811 lines to focused modules, improving maintainability and readability.

## 2026-02-08 - src/features/agent/components/AgentMessageRenderer.tsx

**Split:** `components/CodeBlock.tsx`, `components/MarkdownComponents.tsx`, `components/MarkdownText.tsx`, `hooks/useContentGrouping.ts`, `hooks/useUIActionHandler.ts`, `types.ts`, `constants.ts`
**Improvement:** Decomposed massive rendering component (935 lines) into focused sub-components and hooks. Extracted complex UI action handling (289 lines) and content grouping logic. Main component reduced to 355 lines, improving readability and separation of concerns.
