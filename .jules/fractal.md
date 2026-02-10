## 2026-02-05 - src-tauri/src/mcp/builtin/workspace/code_execution/shell.rs

**Split:** `handlers.rs`, `persistent.rs`, `isolated.rs`, `async_exec.rs`
**Improvement:** Decoupled MCP request handling from core execution logic. Separated persistent shell state management from isolated process execution. Reduced file size from >1100 lines to focused <300 line modules.

## 2026-02-07 - src-tauri/src/session_isolation.rs

**Split:** `types.rs`, `common.rs`, `platforms/mod.rs`, `platforms/windows.rs`, `platforms/linux.rs`, `platforms/macos.rs`, `platforms/unix.rs`, `mod.rs`
**Improvement:** Decoupled platform-specific isolation logic (Linux `unshare`, macOS `sandbox-exec`, Windows job objects) into dedicated modules. Separated types and common utilities. Reduced monolithic file size from ~811 lines to focused modules, improving maintainability and readability.

## 2026-02-09 - src/features/agent/components/AgentMessageRenderer.tsx

**Split:** `types/index.ts`, `hooks/useIsDarkMode.ts`, `hooks/useUIActionHandler.ts`, `components/CodeBlock.tsx`, `components/MarkdownText.tsx`, `config/markdown.tsx`, `utils/contentGrouping.ts`
**Result:** Reduced from ~960 lines to ~280 lines in the main component.
**Improvement:** Decoupled Markdown rendering, UI action handling, and content grouping logic. Extracted large memoized components (`CodeBlock`, `MarkdownText`) and hooks.

## 2026-02-13 - src-tauri/src/mcp/builtin/workspace/code_execution/interactive.rs

**Split:** `handlers.rs`, `security.rs`, `ui.rs`, `mod.rs`
**Result:** Reduced from 948 lines to 657 lines (handlers) + separated logic.
**Improvement:** Decoupled UI generation (HTML/JS) and security (redaction/obfuscation) from MCP command handlers.
