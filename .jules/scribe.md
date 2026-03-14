# Scribe's Journal - Drift Log

## 2024-05-22 - README.md

**Drift:** Linux installation instructions for building from source were missing critical system dependencies (`libglib2.0-dev`, `libgtk-3-dev`, etc.), causing `cargo test` to fail.
**Reality:** Users must install `libglib2.0-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libwebkit2gtk-4.1-dev` on Debian/Ubuntu to build the Tauri backend.

## 2024-05-22 - src/README.md

**Drift:** Contains Python code inside `js` code blocks. Contains typos ("OpneAI"). References potentially non-existent models (`gpt-4.1`, `o4-mini`).
**Reality:** Documentation should use correct language tags and verified model names (e.g., `gpt-4o`, `gpt-4o-mini`).

## 2026-02-06 - src/README.md

**Drift:** Groq section contained Python code labeled as JavaScript. Hallucinated model names found: `claude-sonnet-4-20250514`, `qwen/qwen3-32b`.
**Reality:** Replaced with functional JavaScript examples using `groq-sdk`. Updated models to `claude-3-5-sonnet-20241022`, `deepseek-r1-distill-llama-70b`, `llama-3.3-70b-versatile`.

## 2026-03-01 - README.md

**Drift:** Claims "IndexedDB for local state storage". Lists incomplete built-in tools (missing Knowledge, Skills, etc.).
**Reality:** Local state storage now uses SQLite via SeaORM. Built-in tools include Browser, Workspace (Terminal/Files/Code), Planning, Knowledge, Skills, Playbook, Assistant.

## 2026-03-01 - agents.md

**Drift:** Claims "IndexedDB Storage", "Vite 4.x". References `rmcp` (correct, but clarify version/context). References missing `docs/builtin-tools.md`.
**Reality:** Storage is SQLite via SeaORM. Vite is version 6.x. Documentation for built-in tools is scattered or outdated.

## 2026-03-01 - src-tauri/src/mcp/builtin/README.md

**Drift:** Lists outdated modules (`filesystem.rs`, `sandbox.rs`). Uses incorrect `BuiltinMCPServer` trait signature in examples. Written in Korean (violates project language policy).
**Reality:** Modules are `browser`, `workspace`, etc. Trait signature includes `session_id` and returns `Result<MCPResult, String>`. Documentation must be in English.

## 2026-03-01 - Multiple Files (agents.md, CLAUDE.md, etc.)

**Drift:** References "IndexedDB" and "Dexie" for local storage. Claims "Vite 4.x".
**Reality:** Local storage uses SQLite via SeaORM. Vite is version 6.x.

## 2024-05-22 - CONTRIBUTING.md

**Drift:** References `docs/architecture/overview.md` which does not exist.
**Reality:** The architecture documentation is split. The most comprehensive overview is `docs/architecture/agent-workflow-architecture.md`.

## 2024-05-22 - agents.md

**Drift:** References `docs/architecture/chat-feature-architecture.md` which does not exist.
**Reality:** The file `docs/architecture/agent-workflow-architecture.md` exists and covers the agent workflow and architecture.

## 2026-02-10 - README.md

**Drift:** Supported LLMs list missing Groq, Ollama, Cerebras, Fireworks.
**Reality:** Updated list to match `src/lib/ai-service/types.ts`, excluding the internal placeholder `Empty` provider from user-facing documentation.

## 2026-02-10 - agents.md / CONTRIBUTING.md / docs/README.md

**Drift:** Links to missing files `docs/builtin-tools.md`, `docs/architecture/chat-feature-architecture.md`, `docs/architecture/overview.md`.
**Reality:** Updated links to point to `src-tauri/src/mcp/builtin/README.md`, `agents.md`, or removed if missing.

## 2026-03-01 - src-tauri/src/mcp/builtin/README.md

**Drift:** `BuiltinMCPServer` trait documentation was missing `display_name` and `metadata` methods.
**Reality:** Added methods to trait definition to match `mod.rs`.

## 2026-03-01 - src/README.md

**Drift:** Contained Node.js examples (`process.env`, `process.stdout`, `eval`) in a frontend source directory.
**Reality:** Updated to Frontend-compatible examples (`import.meta.env`, `console.log`) and renamed title to "Frontend AI Integration Examples".

## 2026-03-01 - src-tauri/src/mcp/builtin/README.md

**Drift:** Module structure missing `session_api` and `tests`. Claimed tools use `builtin_` prefix.
**Reality:** Added missing modules. Clarified that tools use simple names (e.g., `readFile`) without prefixes. Updated frontend integration example.

## 2026-03-01 - CONTRIBUTING.md

**Drift:** Referenced `src/lib/ai-service/__tests__/openai.test.ts` which does not exist.
**Reality:** Updated to use `src/lib/ai-service/__tests__/anthropic.test.ts` which exists and passes.

## 2026-03-02 - CLAUDE.md

**Drift:** Linked to missing files `docs/architecture/chat-feature-architecture.md`, `docs/builtin-tools.md`, `docs/external-mcp-integration.md`.
**Reality:** Updated links to point to `docs/architecture/agent-workflow-architecture.md`, `src-tauri/src/mcp/builtin/README.md`, and `docs/architecture/external-mcp-integration.md`.

## 2026-03-02 - agents.md / src-tauri/src/mcp/builtin/README.md

**Drift:** `agents.md` claimed uniform feature structure and used a simplified `get_service_context` example. `builtin/README.md` showed an outdated `get_service_context` default implementation.
**Reality:** Features structure varies; `BrowserServer` context logic is complex; `BuiltinMCPServer` trait returns a formatted default string. Updated docs to reflect reality.

## 2026-03-03 - src-tauri/src/mcp/builtin/README.md / src/lib/utils.ts

**Drift:** `README.md` claimed frontend uses simple tool names. `utils.ts` had implicit documentation on naming.
**Reality:** Agent/Proxy interactions require `builtin_{server}__{tool}` prefix. Updated docs to distinguish between internal (simple) and external (prefixed) naming conventions.

## 2026-03-03 - src/lib/llm-config-manager.ts

**Drift:** Provider map in `getLangchainModelId` missed `gemini` mapping, despite `AIServiceProvider` supporting it.
**Reality:** Added `gemini: 'google-genai'` mapping to align with type definitions.

## 2026-03-03 - README.md

**Drift:** "Key Built-in Tools" list was missing "Content Store".
**Reality:** Added "Content Store" to the list of key built-in tools.

## 2026-02-25 - src/lib/llm-config-manager.ts / src-tauri

**Drift:** `getLangchainModelId` missing mappings for Ollama, Cerebras, Fireworks. Rust types `AgentMessageDto` and command `switch_session` were deprecated but not marked as such in code.
**Reality:** Added missing provider mappings. Added `#[deprecated]` attributes to Rust code and updated comments.

## 2026-02-28 - src-tauri/src/mcp/builtin/error_guidance.rs

**Drift:** Referenced `docs/guides/builtin-tool-best-practices.md` which does not exist.
**Reality:** The file is named `docs/guides/builtin_tool_bp.md`.

## 2026-02-28 - src-tauri/src/mcp/builtin/planning/tools.rs

**Drift:** Referenced `file://workspace/docs/readme.md` in a tool description example which is not a standard file path.
**Reality:** Updated the example to use `file://workspace/README.md`.

## 2026-03-05 - src/lib/workspace-sync-service.ts

**Drift:** JSDoc for `normalizeUnicode` claimed `@param filename` while the actual function argument was `name`.
**Reality:** Updated `@param filename` to `@param name` to match the actual function signature.

## 2024-05-22 - src/lib/token-utils.ts

**Drift:** JSDoc for `selectMessagesWithinContext` claimed `@param options.systemPrompt` and `@param options.toolsJson` while it's actually `@param options` with those properties. Missing `@param modelId`.
**Reality:** Update JSDoc to correctly document the parameters `providerId`, `modelId`, `maxTokens`, and `options` with properties.

## 2024-05-22 - src/context/EditorContext.tsx

**Drift:** JSDoc for `useEditorField` was written in Korean, violating the project's English language policy.
**Reality:** Translated the description and `@param fieldName` documentation to English.

## 2026-03-03 - [src/lib/ai-service/*] **Drift:** [Functions missing JSDoc @param declarations for various properties or deeply nested properties like context.options.modelName] **Reality:** [Function implementations accurately declare their parameters via TypeScript signatures, but JSDocs were missing @param tags for complex objects or were out of sync]

## 2024-05-18 - [src/lib/logger.ts] **Drift:** [`@param` definitions mismatched actual arguments (e.g., `initialize`, `Logger` methods with context overrides)] **Reality:** [Logger methods now accept an optional trailing context string via `...args`, and `getLogger(contextName)` binds that context by appending it as the final argument so callers do not supply it manually; the logging pipeline (including `formatLogMessage`) still interprets a trailing string argument as the log context]

## 2026-03-05 - src/context/EditorContext.tsx

**Drift:** JSDoc for `useEditorField` claimed `@param fieldName - The key of the field to update` while the function uses `fieldName` without the hyphen in description.
**Reality:** Updated to `@param fieldName The key of the field to update`.

## 2026-03-05 - src/hooks/useDebounce.ts

**Drift:** JSDoc contained hyphens in `@param callback - Function` and `@param delay - Delay`.
**Reality:** Removed hyphens to match standard JSDoc style.

## 2026-03-05 - src/hooks/useThrottle.ts

**Drift:** JSDoc contained hyphens in `@param callback - Function` and `@param delay - Minimum delay`.
**Reality:** Removed hyphens to match standard JSDoc style.

## 2026-03-05 - src/lib/utils.ts

**Drift:** JSDoc for `cn` claimed `@param inputs` instead of `@param ...inputs`.
**Reality:** Updated to `@param inputs` and documented that it is a rest parameter.

## 2026-03-05 - src/lib/ai-service/base-service.ts

**Drift:** JSDoc for `constructor` missed `@param config`.
**Reality:** Added `@param config Optional configuration to override the defaults.`

## 2024-10-24 - src/lib/ai-service/ollama-core.ts

**Drift:** Documented param `accumulators` had duplicate/outdated description.
**Reality:** Fixed description.

## 2024-10-24 - src/lib/workspace-sync-service.ts

**Drift:** Missing param `sessionId` in JSDoc for `syncFileToWorkspace`.
**Reality:** Added documentation for `sessionId`.

## 2024-10-24 - src/lib/backend/workspace.ts

**Drift:** Missing param `sessionId` in JSDoc for `workspaceWriteFile`.
**Reality:** Added documentation for `sessionId`.

## 2026-03-08 - src-tauri/src/mcp/builtin/workspace/utils.rs

**Drift:** Code example was ignored and unverified.
**Reality:** Example compiles correctly when tested with `cargo test --doc`.

## 2026-03-08 - src-tauri/src/mcp/service_proxy_manager/creation.rs

**Drift:** Code example was ignored without explanation.
**Reality:** Example is ignored because it requires valid, initialized DB connection and session manager, which is now documented.

## 2026-03-08 - src-tauri/src/mcp/service_proxy_manager/management.rs

**Drift:** Code example was ignored without explanation and had syntactical error.
**Reality:** Example is ignored because it requires initialized connections; also fixed `json!` macro to `serde_json::json!`.

## 2026-03-08 - src-tauri/src/repositories/in_memory_session_repository.rs

**Drift:** Code example was ignored and import was incorrect.
**Reality:** Fixed import statement to accurately reflect the correct module path `in_memory_session_repository` so the example can be successfully verified.

## 2026-03-08 - src/hooks/useDebounce.ts

**Drift:** JSDoc `@param callback` and `@param delay` did not accurately reflect the TypeScript generic signature `T` and missing parameter types in comments.
**Reality:** Updated `@param` to include `{T}` and `{number}` and documented `@template T`.

## 2026-03-08 - src/hooks/useThrottle.ts

**Drift:** JSDoc `@param callback` and `@param delay` did not accurately reflect the TypeScript generic signature `T` and missing parameter types in comments.
**Reality:** Updated `@param` to include `{T}` and `{number}` and documented `@template T`.

## 2026-03-08 - src-tauri/src/[mcp/builtin/utils.rs, mcp/service_proxy_manager/creation.rs, repositories/in_memory_session_repository.rs, utils/json.rs]

**Drift:** Rust documentation generated warnings due to unresolved links (`[validate_path]`, `[call_tool]`) and unclosed HTML tags (`Arc<RwLock>`, `Option<String>`, `Option<T>`).
**Reality:** Fixed the links to correctly resolve to their respective struct methods and wrapped the HTML-like types in backticks (e.g., `Arc<RwLock>`) to eliminate rustdoc warnings.

## 2026-03-14 - src-tauri/src/*
**Drift:** Rust documentation generated 258 `clippy::doc_markdown` warnings due to missing backticks for types, variables, and code keywords across the rust codebase.
**Reality:** Ran `cargo clippy --fix` to wrap terms like `SQLite`, `SeaORM`, `AppHandle`, and variable names in backticks to conform to Rust doc standards and remove warnings.
