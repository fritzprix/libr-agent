# Critical Clarification Questions for Agentic Workflow Migration

Based on a deep code review of `src/context/ChatContext.tsx`, `src/hooks/use-ai-service.ts`, and the Rust backend structure, the following questions must be answered to elaborate the migration plan.

## 1. The "Headless" Execution Problem (Critical)

The proposal states: _"TS provides the dependency layer (LLM Provider Integration) to Rust"_.

**Context:** If the user minimizes the app (suspending the WebView) or closes the window (to run in tray mode), the TypeScript runtime stops.

- **Q1:** Is the goal for agents to run **truly independently** (e.g., while the app is in the system tray or the window is closed)?
  - _If YES:_ We cannot rely on TS for LLM calls. We must port `AIServiceFactory` and provider logic (OpenAI, Anthropic, etc.) to Rust. Are we prepared for this effort?
  - _If NO:_ Then the "Background Agent" is just a "Hidden UI Agent". Is this acceptable?

> **User Decision:** The Tauri App lifecycle is tied to the WebView window. Closing the window terminates the app. Therefore, we do NOT need to support agent execution when the window is closed.
>
> **Implication:** The TypeScript runtime is guaranteed to be available whenever the Rust backend is running. This validates the "Hybrid" architecture where Rust orchestrates the workflow but delegates LLM calls (and potentially Web Tools) back to TypeScript via IPC.
>
> **Requirement:** To solve the original problem of "Session Switching Interruption", the TypeScript "Service Layer" (LLM/Tools) must be moved **outside** of React Page Components (which unmount) and into a global context or singleton that persists for the app's lifetime.

## 2. The "Two Worlds" Tooling Split

**Context:** The codebase currently has **Web MCP** tools (running in Browser/WebWorkers, e.g., Page Analysis) and **Native MCP** tools (running via `rmcp` in Rust).

- **Q2:** How should a Background Agent (running in Rust) handle **Web MCP tools**?
  - _Scenario:_ A user asks a background agent to "Summarize this webpage". Rust has no DOM access.
  - _Option A:_ Disable Web Tools for background agents (Feature degradation).
  - _Option B:_ Re-implement Web Tools in Rust (e.g., using `headless_chrome` crate).
  - _Option C:_ Keep the UI open and use it as a "Tool Server" for the backend (Fragile).

> **User Decision:**
>
> 1.  **Immediate Solution (IPC Proxy):** Since the App Window remains open (per Q1), we can expose Web MCP tools to the Rust backend via a Global Context/Provider. Rust can request tool execution via IPC, and the Frontend will relay it to the Web Worker.
> 2.  **Long-term Goal (Rust Porting):** Many Web MCP tools are purely logical (e.g., Planning) or computational (e.g., BM25/Search). These should be gradually ported to Rust for better performance.
>     - _Note:_ Rust has excellent libraries for BM25 (e.g., `tantivy`), making the migration of search-related tools highly beneficial.
>
> **Implication:** We need a **"Reverse Tool Bridge"**.
>
> - Current: Frontend calls Rust Tools.
> - New: Rust calls Frontend Tools.

## 3. Database & Data Sovereignty

**Context:** Currently, `SessionContext.tsx` uses `IndexedDB` as the source of truth. `session.rs` only manages file paths. The proposal says _"Rust Backend creates the session in DB"_.

- **Q3:** What is the **Source of Truth** for message history?
  - _Option A (Migration):_ Move ALL storage to Rust (e.g., SQLite or JSONL files). The Frontend becomes a "View" that fetches data from Rust.
  - _Option B (Sync):_ Keep IndexedDB as primary, but sync to Rust. (High risk of state drift).
  - _Option C (Hybrid):_ Rust manages "Active Workflow" state, but final storage is sent back to Frontend. (Violates independence).

> **User Decision:** **Option A (Rust SQLite)**.
> Ideally, all data should be migrated to Rust's SQLite. This ensures a single source of truth and better performance for backend operations.
>
> **Implication:**
>
> - Need to introduce `rusqlite` crate.
> - Need a migration strategy for existing IndexedDB data.
> - Frontend `SessionContext` will change from reading IndexedDB to calling Tauri Commands (e.g., `get_session_history`).

## 4. State Synchronization Protocol

**Context:** `ChatContext.tsx` handles high-frequency updates (streaming tokens).

- **Q4:** What is the **Streaming Strategy**?

> **User Clarification:**
> The sequence diagram in `idea.md` already defines this correctly:
>
> 1.  **Streaming stays in Frontend:** `useAgentSession` (or the new Global Context) handles the LLM stream and updates the UI directly.
> 2.  **Rust gets Final State:** Rust (`AgentSessionManager`) is only notified when the message is _complete_ (`pushMessage`).
> 3.  **Flow Control:** Rust decides the next step (e.g., Tool Execution) _after_ receiving the full message.
>
> **Conclusion:** We do **not** need to stream tokens from Rust to TS. The original design avoids the IPC performance bottleneck.
> **Action Item:** Ensure the Global TS Service (`LLMServiceProviderContext`) implements this "Stream locally, Report completion to Rust" pattern.

## 5. Migration & Backward Compatibility

**Context:** `useAIService.ts` contains complex logic for context window management, token counting, and error handling.

- **Q5:** Do we **Port or Wrap** existing logic?
  - _Port:_ Rewrite `token-utils.ts`, `message-preprocessor.ts`, etc., in Rust. (Cleanest, but high effort).
  - _Wrap:_ Keep the logic in TS, calculate the "Prompt" in TS, send the _final_ prompt to Rust to execute. (Easier, but keeps the TS dependency).

> **User Decision:** **Wrap First, Port Later.**
>
> - Goal: Progressive migration. Coexistence of new and old features.
> - Strategy: Use existing TS logic where possible to avoid rewriting complex token/preprocessing logic immediately.
> - Code Quality: Even if wrapping, the interface should be clean and elegant, fitting the new structure.

## 6. Security Model

**Context:** Background agents running autonomously present higher risks.

- **Q6:** How do we handle **Human-in-the-Loop** permissions for background agents?
  - If a background agent tries to `delete_file`, and the UI is closed, does it fail? Does it wait? Does it send a system notification?

> **User Decision:** **Monitoring is Sufficient.**
>
> - Users can periodically switch sessions to monitor agents.
> - No strict "Approval UI" is needed for the MVP.
> - Trust the user's intent when running an agent.

## Recommended Decision Path

To proceed, I recommend we agree on the **"Rust-First" Architecture**:

1.  **Storage**: Rust owns the data (SQLite/File).
2.  **Execution**: Rust runs the Agent Loop.
3.  **Network**: Rust makes the HTTP calls (Port LLM Clients).
4.  **UI**: React is just a visualization layer.
