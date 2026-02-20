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
