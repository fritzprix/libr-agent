## 2026-02-05 - src-tauri/src/mcp/builtin/workspace/code_execution/shell.rs

**Split:** `handlers.rs`, `persistent.rs`, `isolated.rs`, `async_exec.rs`
**Improvement:** Decoupled MCP request handling from core execution logic. Separated persistent shell state management from isolated process execution. Reduced file size from >1100 lines to focused <300 line modules.

## 2026-02-07 - src-tauri/src/session_isolation.rs

**Split:** `types.rs`, `common.rs`, `platforms/mod.rs`, `platforms/windows.rs`, `platforms/linux.rs`, `platforms/macos.rs`, `platforms/unix.rs`, `mod.rs`
**Improvement:** Decoupled platform-specific isolation logic (Linux `unshare`, macOS `sandbox-exec`, Windows job objects) into dedicated modules. Separated types and common utilities. Reduced monolithic file size from ~811 lines to focused modules, improving maintainability and readability.

## 2026-02-09 - src-tauri/src/session.rs

**Split:** `types.rs`, `manager.rs`, `mod.rs`
**Improvement:** Decoupled data types (`SessionWorkspaceInfo`, `SessionStats`) from core logic (`SessionManager`). Separated global initialization and re-exports into `mod.rs`. Improved maintainability by creating a dedicated module directory structure for session management.

## 2026-02-09 - src/features/agent/components/AgentMessageRenderer.tsx

**Split:** `types/index.ts`, `hooks/useIsDarkMode.ts`, `hooks/useUIActionHandler.ts`, `components/CodeBlock.tsx`, `components/MarkdownText.tsx`, `config/markdown.tsx`, `utils/contentGrouping.ts`
**Result:** Reduced from ~960 lines to ~280 lines in the main component.
**Improvement:** Decoupled Markdown rendering, UI action handling, and content grouping logic. Extracted large memoized components (`CodeBlock`, `MarkdownText`) and hooks.

## 2026-02-11 - src/lib/ai-service/gemini.ts

**Split:** `config.ts`, `mapper.ts`, `models.ts`, `service.ts`, `stream.ts`, `types.ts`, `index.ts`
**Result:** Reduced from ~1120 lines to ~250 lines in the main service class.
**Improvement:** Decoupled message conversion (Gemini format), stream processing (chunk parsing, tool calls), and model management logic from the main service class. Centralized configuration and types.

## 2026-02-12 - src-tauri/src/lib.rs

**Split:** `src-tauri/src/lifecycle/` (`database.rs`, `repositories.rs`, `app_setup.rs`, `settings.rs`, `mod.rs`)
**Improvement:** Decoupled application startup logic (database initialization, repository setup, app configuration) from the main library entry point. Reduced `lib.rs` from ~800 lines to ~250 lines, improving readability and separation of concerns.

## 2026-02-12 - src-tauri/src/mcp/builtin/workspace/code_execution/interactive.rs

**Split:** `security.rs`, `ui.rs`, `handlers.rs`
**Result:** Reduced from 959 lines to 4 lines (module definition).
**Improvement:** Decoupled security (redaction/obfuscation), UI generation (HTML), and MCP request handling logic into dedicated modules.

## 2026-02-12 - src-tauri/src/agent/llm.rs

**Split:** `types.rs`, `prompt.rs`, `completion.rs`, `response.rs`, `mod.rs`
**Improvement:** Decoupled LLM request handling, response processing, and prompt construction into focused modules. Extracted `CompletionRequest` DTO and prompt building logic. Reduced file size from >800 lines to composed sub-modules.

## 2026-02-13 - src-tauri/src/services/interactive_browser_server.rs

**Split:** `types.rs`, `constants.rs`, `utils.rs`, `id_gen.rs`, `mod.rs`
**Result:** Reduced from ~845 lines to ~450 lines in the main module.
**Improvement:** Decoupled types (`BrowserSession`), constants (`INIT_SCRIPT`), stateless utilities (`check_url_status`, `validate_and_normalize_url`), and ID generation from the main server logic. Improved readability and testability.
