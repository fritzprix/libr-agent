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

## 2026-02-11 - src/lib/ai-service/gemini.ts

**Split:** `config.ts`, `mapper.ts`, `models.ts`, `service.ts`, `stream.ts`, `types.ts`, `index.ts`
**Result:** Reduced from ~1120 lines to ~250 lines in the main service class.
**Improvement:** Decoupled message conversion (Gemini format), stream processing (chunk parsing, tool calls), and model management logic from the main service class. Centralized configuration and types.
