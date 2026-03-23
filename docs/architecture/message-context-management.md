# Message Context Management

## Overview

Agent V2 message context management is now **Rust-orchestrated**.

The old React-led flow is no longer the source of truth for agent sessions. Rust owns:

- message stack preparation
- system prompt assembly
- compact-summary reinjection
- token estimation and context selection
- compaction trigger / persistence / restoration

The frontend still does one important job: it acts as the **LLM execution bridge**. Rust emits completion and compaction requests through Tauri events, the frontend calls the provider SDK, and the result is sent back to Rust.

---

## Current Ownership Split

### Rust backend owns orchestration

Primary files:

- `src-tauri/src/agent/llm/completion.rs`
- `src-tauri/src/agent/llm/prompt.rs`
- `src-tauri/src/agent/llm/context_selector.rs`
- `src-tauri/src/agent/llm/token_utils.rs`
- `src-tauri/src/agent/session_manager.rs`
- `src-tauri/src/agent/lifecycle/{creation,cache,management}.rs`
- `src-tauri/src/repositories/compact_context_repository.rs`

Rust is responsible for:

- draining pending user messages into the in-memory session cache
- filtering out recovery tombstones from normal LLM context
- building the stable prompt prefix and volatile session context
- injecting any saved compact summary back into the message stack
- calculating grounded token usage
- selecting the final message subset for the request
- deciding whether to trigger async compaction
- persisting compact summaries and restoring them on session load/resume

### Frontend owns provider API execution

Primary files:

- `src/context/llm/useLLMListener.ts`
- `src/context/llm/useLLMExecution.ts`
- `src/lib/ai-service/base-service.ts`
- `src/lib/ai-service/anthropic.ts`
- `src/lib/backend/agent-commands.ts`

The frontend is responsible for:

- listening for `llm:completion-request`
- calling the selected provider SDK
- streaming and normalizing the assistant response
- returning the result through `agent_handle_llm_response`
- listening for `llm:compact-request`
- executing `service.compact(...)`
- returning the summary through `agent_handle_compact_response`

This is a bridge role, not an orchestration role.

---

## High-Level Request Flow

```text
User message
  ↓
Rust workflow starts
  ↓
request_llm_completion()
  ├─ drain pending events into session cache
  ├─ load/build stable prompt + volatile session context
  ├─ resolve @references in user messages
  ├─ inject saved compact summary if present
  ├─ compute grounded token usage
  ├─ optionally trigger async compaction
  ├─ select final messages within context
  └─ emit llm:completion-request
        ↓
Frontend useLLMListener
  ├─ execute provider SDK call
  ├─ apply retry/fallback logic
  └─ return assistant message via agent_handle_llm_response
        ↓
Rust continues workflow / tool execution
```

---

## System Prompt Assembly

System prompt assembly is handled in `src-tauri/src/agent/llm/prompt.rs`.

### Stable / volatile split

`build_session_system_prompt_split()` returns:

- `stable_prompt`
- `session_context` (optional volatile suffix)

The stable prefix is cached in `AgentSession.cached_stable_prompt`.

### Stable prefix contents

Built by `build_stable_prefix()`:

1. agent identity / base system prompt
2. persona template from the first matching file in:
   - `.github/SOUL.md`
   - `SOUL.md`
   - `.github/soul.md`
   - `soul.md`
3. workspace instructions from the first matching file in:
   - `agents.md`
   - `AGENTS.md`
   - `CLAUDE.md`
   - `GEMINI.md`
4. session metadata label (`## Session Context`)

### Volatile suffix contents

Built by `build_volatile_sections()`:

5. `ContextRegistry` output
6. builtin tool service contexts from `proxy.get_service_contexts()`

Service contexts are sorted by `tool_id` before concatenation so the prompt byte sequence remains deterministic across requests.

### Important caveat: only `context_prompt` reaches the LLM

Builtin tool service contexts may contain:

- `context_prompt`
- `structured_state`

Only `context_prompt` is appended to the system prompt. `structured_state` does **not** reach the model.

If the AI must see a value, it must be written into `context_prompt` as plain text.

---

## Stable Prompt Cache Behavior

The stable prefix is intentionally cached for the session lifetime because it contains session-immutable data.

This improves provider-side prefix caching, but it has a tradeoff:

- edits to workspace instruction files during a live session are **not** reflected immediately
- they are picked up after cache invalidation, config update, or session resume

This behavior is documented directly in `prompt.rs` and is intentional.

---

## Message Stack Preparation

Main implementation: `src-tauri/src/agent/llm/completion.rs`

Before sending an LLM request, Rust prepares the message stack in this order:

1. validate session state
2. drain pending user messages from `pending_events`
3. append drained messages into the in-memory session cache
4. emit `MessageAdded` events for those drained messages
5. read cached session messages
6. drop messages with `source == "recovery"`
7. resolve `@type:arg` references in user messages
8. merge consecutive user messages when recovery produced an unanswered tail
9. inject saved compact summary if one exists and still matches the current stack
10. run context selection
11. emit `llm:completion-request`

### Recovery tombstones

Crash recovery may inject synthetic `tool` error messages with `source: "recovery"` to close orphaned tool calls. These are useful for UI and workflow recovery, but are excluded from normal LLM context assembly.

---

## Compaction Architecture

Compaction is session-scoped and persisted.

### In-memory runtime state

Defined in `src-tauri/src/agent/state.rs`:

- `compact_context: Arc<RwLock<Option<CompactContextRecord>>>`
- `compact_in_flight: Arc<AtomicBool>`
- `last_compacted_tail_id: Arc<RwLock<Option<String>>>`

### Persisted record

Stored by `SqliteCompactContextRepository` in `compact_contexts`:

- `id`
- `session_id`
- `from_id`
- `to_id`
- `summary`
- `created_at`

Repository behavior:

- one record per session
- upsert on `session_id`
- newest compact summary replaces the previous one

---

## Compaction Lifecycle

### Step A: summary reinjection

At the start of each request, `request_llm_completion()` checks `session.compact_context`.

If a valid record exists and `to_id` is still present in the current message stack:

- Rust creates a synthetic user message with:
  - `id = compact-summary-{session_id}`
  - `source = "compact-summary"`
  - content:

    ```text
    ### Previous Conversation Summary

    {summary}
    ```

- Rust replaces the compacted prefix with:
  - `[compact-summary-message] + [tail-messages-after-to_id]`

If `to_id` is missing, the in-memory compact cache is invalidated as stale.

### Step B: trigger async compaction

If `contextStrategy == "compact"` and grounded token usage exceeds the compact threshold:

- threshold = `floor(effective_limit * 0.9)`
- Rust computes `split_idx = find_compaction_split_index(messages)`

Current implementation detail:

- `find_compaction_split_index()` currently returns `messages.len()`
- meaning the whole current stack is compacted
- the future "tail" is whatever new messages arrive after compaction is triggered

#### Guard G1: `compact_in_flight`

Rust uses `compare_exchange(false, true, ...)` on `compact_in_flight`.

This prevents two concurrent requests from triggering duplicate compaction work.

#### Guard G2: `last_compacted_tail_id`

Rust stores the last message ID from the stack when compaction is triggered.

If the next candidate request sees the same tail ID, it skips compaction because nothing new has arrived since the last trigger.

### Step C: frontend LLM summary call

Rust emits:

- `llm:compact-state` with `compacting: true`
- `llm:compact-request`

Frontend `useLLMListener.ts` receives the compact request, calls:

```ts
service.compact(messages, { modelName: model });
```

and returns the summary through:

- `agent_handle_compact_response` on success
- `agent_handle_compact_error` on failure

### Step D: store result and clear in-flight state

`AgentSessionManager.handle_compact_response()`:

1. builds a `CompactContextRecord`
2. stores it in-memory
3. upserts it in SQLite
4. clears `compact_in_flight`

On error, Rust still clears `compact_in_flight` so a later request can retry.

---

## Compaction State Events

The frontend also listens for:

- `llm:compact-state`

Current use:

- expose `compacting: boolean` to the UI
- show compacting state without giving ownership of compaction orchestration to React

This event is Rust-owned state, not a frontend guess.

---

## Context Selection

Context selection is implemented in `src-tauri/src/agent/llm/context_selector.rs`.

The selector is still used for both strategies:

- `contextStrategy == "compact"`
- `contextStrategy != "compact"` (window-style fallback / legacy path)

### Shared selection behavior

`select_messages_within_context()` does the following:

1. batch oversized assistant tool-call messages with `batch_tool_calls_in_messages()`
2. estimate token cost for:
   - system prompt
   - tool schema payload
   - pinned first user message
3. compute a calibrated token budget using grounded API usage if available
4. walk backward from newest to oldest
5. stop when:
   - token budget is exceeded, or
   - `max_messages` is hit
6. for certain providers, call `remove_incomplete_tool_chains()`
7. prepend the pinned first user message
8. merge pinned + selected first user message if they would otherwise be consecutive

### Provider-specific integrity guard

Incomplete tool-chain cleanup is currently enabled for:

- Anthropic
- Gemini
- OpenAI
- OpenRouter
- Groq

### Gemini batching exception

Gemini uses a very high `max_tool_calls_per_message` value to avoid splitting turns that must remain atomic.

### Compact mode vs window mode

When `contextStrategy == "compact"`:

- Rust may trigger async compaction
- Rust still runs the selector with `max_messages = None`
- the selector remains the final guardrail against oversized requests

When `contextStrategy != "compact"`:

- Rust skips compaction triggering
- Rust runs the selector with `max_messages = windowSize`

So the old sliding-window path still exists, but it is no longer the primary Agent V2 design.

---

## Token Estimation

Implemented in `src-tauri/src/agent/llm/token_utils.rs`.

### Core functions

- `estimate_text_tokens()`
- `estimate_tokens_bpe()`
- `calculate_grounded_total_tokens()`
- `calculate_compact_threshold()`

### Important behavior

Token estimates use `cl100k_base` when available, with a char-based fallback.

`calculate_grounded_total_tokens()` looks for the most recent assistant message with API-reported usage and uses that as a calibration anchor. This keeps local BPE estimation from drifting too far after long runs.

There is also an explicit guard for compact-summary insertion so stale pre-compact usage does not keep the estimate artificially inflated forever.

---

## Persistence and Restoration

Compaction is restored in multiple lifecycle paths, not just one.

### Session creation

`src-tauri/src/agent/lifecycle/creation.rs`

- loads compact context from the repository
- stores it in the new `AgentSession`

### Cache initialization

`src-tauri/src/agent/lifecycle/cache.rs`

- loads recent messages
- loads compact context
- hydrates both into the in-memory session state

### Session resume / active-session refresh

`src-tauri/src/agent/lifecycle/management.rs`

- reloads compact context
- updates in-memory runtime state for the resumed session

### Safety lookup

`src-tauri/src/agent/session_manager.rs`

- `get_compact_context()` checks in-memory state first
- falls back to the repository if needed

This means compact summaries are persistent session state, not ephemeral frontend state.

---

## Frontend Provider Injection Strategy

Rust sends `system_prompt` and `session_context` separately in `CompletionRequest`.

The frontend provider layer decides how to inject them.

### Default behavior

`src/lib/ai-service/base-service.ts`

Base services can concatenate the stable prompt and volatile session context into one system prompt.

### Anthropic-specific behavior

`src/lib/ai-service/anthropic.ts`

Anthropic uses `VOLATILE_CONTEXT_MARKER` to split the prompt into:

- cached stable prefix
- uncached volatile suffix

The stable block is marked with:

```ts
cache_control: {
  type: 'ephemeral';
}
```

This is the current prompt-cache optimization path.

### OpenAI-specific behavior

`src/lib/ai-service/openai.ts`

OpenAI overrides `prepareContextInjection()` differently:

- keep the stable system prompt untouched
- inject `sessionContext` as an ephemeral tail `user` message

This preserves a stable system-prompt prefix while still giving the model the latest session state.

---

## What Changed from the Old React-Led Design

The old documentation described compaction as if React owned:

- token measurement
- compaction orchestration
- overflow waiting
- message stack rebuilding

That is no longer true for Agent V2.

### Old assumption

- React hook decides when to compact
- React owns compact waiters / overflow handling
- token utils in frontend are the main path

### Current reality

- Rust owns the orchestration loop
- Rust triggers compaction and persists summaries
- frontend only performs the provider API calls
- the final message stack is assembled in Rust

Frontend utilities and legacy paths may still exist for non-Agent-V2 flows, but they are not the authoritative architecture for agent sessions.

---

## Current Limitations

These are the limitations of the **implemented** system, not the old proposal.

1. **No hard wait at 100%**
   - The earlier design idea mentioned waiting for compaction if the context fully overflowed.
   - Current implementation does not block on compact completion.
   - It triggers async compaction and still relies on the selector as the immediate safety rail.

2. **Whole-stack compaction split**
   - `find_compaction_split_index()` currently returns `messages.len()`.
   - There is no smarter semantic split yet.

3. **Single compact record per session**
   - Each new summary replaces the previous session record.
   - There is no layered compact-history stack.

4. **Prompt cache wins depend on provider injection strategy**
   - Rust provides a stable/volatile split.
   - Actual prefix-cache benefit still depends on the frontend provider implementation.

5. **`structured_state` is invisible to the model**
   - If tool authors put critical values only in structured JSON, the model will not see them.

---

## Quick Reference

### Rust entry points

- `request_llm_completion()` — `src-tauri/src/agent/llm/completion.rs`
- `build_session_system_prompt_split()` — `src-tauri/src/agent/llm/prompt.rs`
- `select_messages_within_context()` — `src-tauri/src/agent/llm/context_selector.rs`
- `handle_compact_response()` — `src-tauri/src/agent/session_manager.rs`

### Frontend bridge points

- `useLLMListener()` — `src/context/llm/useLLMListener.ts`
- `handleLLMResponse()` — `src/lib/backend/agent-commands.ts`
- `handleCompactResponse()` — `src/lib/backend/agent-commands.ts`

### Persistence

- `SqliteCompactContextRepository` — `src-tauri/src/repositories/compact_context_repository.rs`

---

## Bottom Line

If you are documenting or modifying Agent V2 message context behavior, assume this:

- **Rust owns context management**
- **frontend executes model calls**
- **compact summaries are persisted session state**
- **the selector still remains the final size guard**

If a document says React is the primary compaction orchestrator for agent sessions, that document is outdated.
