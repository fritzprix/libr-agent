## SynapticFlow — App Architecture & Runtime Flow

This document summarizes the frontend (`src/`) and backend (`src-tauri/`) structure and explains how the application initializes and routes requests at runtime based on `src/app/App.tsx`.

It is meant as a concise guide for contributors and maintainers to quickly understand the main responsibilities, feature wiring, and where to look for implementation details.

---

## 1. High-level overview

- Frontend: `src/` — React + TypeScript (Vite), UI components, feature modules, contexts/providers, hooks, and examples.
- Backend: `src-tauri/` — Rust (Tauri) code that manages native MCP servers, builtin MCP servers (filesystem, sandbox), and exposes Tauri commands.

The app is a desktop client that integrates multiple MCP (Model Context Protocol) tool sources:
- External MCP servers launched/managed via Tauri (stdio-based MCPs).
- Built-in MCP servers implemented in Rust (filesystem, sandbox/code-execution, etc.).
- Web-based MCP tools running in WebWorkers (web-mcp).

The frontend composes providers that expose these tool sources and utility services to UI components (chat, tools, session/files). The chat UI can call tools and include their results in assistant conversations.

---

## 2. Project layout (important folders)

- `src/`
  - `app/` — App entry and layout (`App.tsx`, `main.tsx`, global styles).
  - `components/` — Reusable UI components and `shadcn/ui` wrappers.
  - `config/` — Static config files (e.g., `builtin-mcp-servers.json`, `llm-config.json`).
  - `context/` — React contexts providing global services (MCPServerContext, WebMCPContext, BuiltInToolContext, SessionContext, Settings, etc.).
  - `features/` — Feature folders (chat, assistant, prompts, session, settings).
  - `hooks/` — Reusable hooks (use-mcp-server, use-web-mcp, use-unified-mcp, use-ai-service, etc.).
  - `lib/` — Service layer, utilities, Tauri client (`tauri-mcp-client.ts`), MCP types, ai-service implementations.
  - `models/` — TypeScript models (chat messages, attachments, llm-config).
  - `examples/` — Example components (e.g., `BuiltInToolsExample.tsx`).

- `src-tauri/`
  - `src/` — Tauri Rust source (commands, MCP manager).
  - `src-tauri/src/mcp/builtin/` — Built-in MCP servers implemented in Rust:
    - `filesystem.rs` — read/write/list with path validation and file-size limits.
    - `sandbox.rs` — executes Python/TypeScript in a temporary workspace with timeouts.
    - `utils.rs` — security helper (path cleaning, constants, tests).
    - `mod.rs` — registry and trait for builtin servers.
  - `Cargo.toml`, `tauri.conf.json` — Tauri/Rust config and dependencies.

---

## 3. App initialization & provider wiring (`src/app/App.tsx`)

The top-level `App` component composes the application providers and routes. Order matters because some contexts rely on others. Key providers and their roles are:

- `SettingsProvider` — persisted app settings, API keys, feature flags.
- `SchedulerProvider` — scheduled jobs or background tasks.
- `WebMCPProvider` — web MCP tool manager (used for `content-store`), can auto-load specified servers and web tools.
- `MCPServerProvider` — manages external MCP servers (Tauri-launched stdio MCP processes) and provides execute/connection APIs.
- `BuiltInToolProvider` — new provider that lists and exposes builtin Tauri MCP tools (filesystem, sandbox) via `tauriMCPClient` and merges them into the available tool list.
- `AssistantGroupProvider` / `AssistantContextProvider` — assistant/agent storage and selection.
- `SessionContextProvider` + `SessionHistoryProvider` — session lifecycle and conversation history.
- `ResourceAttachmentProvider` — attachment management (upload/download content-store integration). Provides `addFile`, `removeFile`, and session file listings.
- `ModelOptionsProvider` — model selection and LLM providers configuration (local vs external).

UI-level wrappers:
- `SidebarProvider` — UI sidebar state.

Routing: `Routes` maps paths to containers:
- `/`, `/chat/single` → `SingleChatContainer` (single-assistant chat)
- `/chat/group` → `GroupChatContainer` (multi-assistant/group chat)
- `/assistants`, `/assistants/groups` → assistant management UIs
- `/history` → conversation history

The `AppHeader` and `AppSidebar` hosts controls that interact with these contexts (open settings, switch assistant, create session, start/stop MCP servers, etc.).

Key point: the providers connect the UI to multiple MCP tool sources so the chat UI and tools can dispatch tool calls seamlessly.

---

## 4. Core features and where to find them

- Tool Integration (Unified):
  - Frontend hook: `src/hooks/use-unified-mcp.ts` — resolves a tool name to its backend (tauri MCP server, web worker, or builtin) and routes execution.
  - Tauri client: `src/lib/tauri-mcp-client.ts` — exposes `listBuiltinTools`, `callBuiltinTool`, `listAllToolsUnified`, `callToolUnified` (invokes Tauri commands).
  - Builtin providers: `src/context/BuiltInToolContext.tsx` — loads builtin tools and exposes them via context.

- Content Store (session file attachments):
  - Web MCP module (frontend): `src/lib/web-mcp/modules/content-store.ts` (used via `WebMCPProvider`).
  - ResourceAttachmentContext: `src/context/ResourceAttachmentContext.tsx` — upload, remove, list session files and refresh session files.
  - UI: `SessionFilesPopover` (added in `Chat.tsx`) shows session file list and file preview with a dialog to view contents.

- Built-in Servers (Rust):
  - Filesystem (`builtin.filesystem`): safe file reads/writes and directory listing with `SecurityValidator` and max file size limits.
  - Sandbox (`builtin.sandbox`): executes Python/TypeScript code in a temp dir with timeouts and environment isolation.
  - Registry: `src-tauri/src/mcp/builtin/mod.rs` provides `list_all_tools` and `call_tool` for unified listing and calls.

- Chat & System Prompt:
  - Chat components use `ChatContext`, `SessionContext`, and `AssistantContext` to manage messages and tool calls.
  - `BuiltInToolsSystemPrompt.tsx` builds a dynamic system prompt that lists available tools and attached files so the assistant can use them.

- LLM Providers & AI Services:
  - Multiple AI service integrations exist under `src/lib/ai-service/` (Groq, OpenAI, Anthropic, local/placeholder implementations).
  - `ModelOptionsProvider` / `llm-config.json` let users select providers and models. The design avoids locking users into a single LLM provider and supports adding local LLMs.

---

## 5. Typical runtime flows

1. App start
   - `main.tsx` renders `App` → providers initialize. `WebMCPProvider` may auto-load `content-store` web MCP server.
   - `BuiltInToolProvider` calls `tauriMCPClient.listBuiltinTools()` via Tauri to retrieve builtin tools and merges them into `availableTools`.

2. User opens a chat and triggers a tool call
   - UI (Tool Caller) constructs an MCP tool call object `{ id, type:'function', function: { name, arguments } }`.
   - `use-unified-mcp` or `BuiltInToolContext.executeToolCall` determines the backend:
     - If tool name starts with `builtin.` → route to `tauriMCPClient.callBuiltinTool` (Tauri backend builtin server).
     - If tool belongs to web worker tools → `WebMCP` executeCall.
     - Otherwise → execute via external MCP server managed by `MCPServerProvider`.
   - Result (standard MCPResponse) returns to UI and is optionally stored as a tool result message in chat history.

3. Uploading / using session files
   - `ResourceAttachmentProvider.addFile` uploads to the content-store (web MCP), records reference, and triggers `refreshSessionFiles()`.
   - `BuiltInToolsSystemPrompt` reads session files to include metadata/previews in the assistant system prompt.

4. Executing sandboxed code
   - Frontend calls a tool like `builtin.sandbox__execute_python` with code and timeout.
   - Tauri receives `call_builtin.tool`, the builtin sandbox writes code in a temp dir, runs `python3` (or ts-node), enforces timeout, returns output as MCPResponse.

---

## 6. Security notes & suggestions

- The Rust `SecurityValidator` and sandbox timeouts are good first steps. Additional hardening is recommended before exposing sandbox to untrusted inputs:
  - Canonicalize paths and validate symlink escapes.
  - Consider seccomp (Linux), rlimits, or a WASM-based execution environment for stronger isolation.
  - Limit file sizes and execution memory/CPU in sandbox.
  - Ensure logging and error propagation don't leak secrets.

---

## 7. Where to look for implementation details

- Frontend contexts and wiring: `src/context/*.tsx` (especially `BuiltInToolContext.tsx`, `MCPServerContext.tsx`, `WebMCPContext.tsx`, `ResourceAttachmentContext.tsx`).
- Unified tool routing: `src/hooks/use-unified-mcp.ts` and `src/lib/tauri-mcp-client.ts`.
- Chat UI and system prompt: `src/features/chat/Chat.tsx`, `src/features/prompts/BuiltInToolsSystemPrompt.tsx`.
- Builtin server code (Rust): `src-tauri/src/mcp/builtin/*` (filesystem, sandbox, utils, mod).

---

## 8. Next steps I can perform (pick one)
- Update `README.md` to reflect implemented builtin tools (content-store, filesystem, sandbox) and the project's goals (user-friendly tool integration, multi-LLM support).
- Run a TypeScript typecheck (`pnpm build` / `tsc`) and `cargo check` to surface compile-time issues in Rust (I can do this and fix low-risk problems like Path→OsStr conversions).
- Add small unit tests for `SecurityValidator` symlink/canonicalization checks.

If you want, I can update `README.md` next using this analysis as a basis. Tell me which of the next steps to run now.
