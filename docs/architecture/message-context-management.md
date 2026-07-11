# Message Context Management

## Overview

Agent V2 message context management is fully Rust-orchestrated.

The frontend still executes provider SDK calls, but it is no longer the authority
for:

- request fit / no-fit decisions
- compaction triggering
- compacted range selection
- compact summary persistence
- invalid-state rejection

If you need the normative rules, read
[`docs/specs/message-compaction.md`](../specs/message-compaction.md) first. This
document is only the architecture guide.

---

## Ownership Split

### Rust owns context control

Rust is the source of truth for Agent V2 context management.

Primary areas:

- `src-tauri/src/agent/llm/completion/`
- `src-tauri/src/agent/llm/prompt.rs`
- `src-tauri/src/agent/llm/token_utils.rs`
- `src-tauri/src/agent/llm/response.rs`
- `src-tauri/src/agent/session_manager/compact.rs`
- `src-tauri/src/repositories/compact_context_repository.rs`

Rust is responsible for:

- preparing the message stack
- assembling stable prompt + volatile session context
- injecting persisted compact summaries
- computing conservative preflight token estimates
- deciding whether a normal request fits
- deciding whether compaction can proceed
- persisting prompt-token checkpoints on messages
- persisting compact summaries
- rejecting invalid overflow states

### Frontend owns provider execution

Primary areas:

- `src/context/llm/useLLMListener.ts`
- `src/context/llm/useLLMExecution.ts`
- `src/lib/ai-service/`
- `src/lib/backend/agent-commands.ts`

The frontend is responsible for:

- listening for `llm:completion-request`
- calling the selected provider SDK
- listening for `llm:compact-request`
- executing the summary request
- returning completion / compact responses back to Rust

That is a bridge role, not a policy role.

---

## Core Runtime Model

There are two persistent pieces of context state:

1. **Prompt-token checkpoints on messages**
   - `message.promptTokens`
   - stored on the **last submitted input message** of a successful request
   - used as grounded anchors for later preflight compaction decisions
2. **Compact summary record**
   - stored in `compact_contexts`
   - anchored by `to_id`
   - means "history through `to_id` is represented by this summary now"

There is also one important runtime-only bridge field:

- `last_submitted_input_message_id`

That field exists only to connect request emission time to response handling time
so the backend can stamp provider `usage.promptTokens` onto the correct message.

---

## High-Level Request Flow

```text
User message
  ↓
Rust prepares request state
  ├─ loads cached messages
  ├─ builds stable prompt + volatile session context
  ├─ injects compact summary if present
  ├─ computes conservative preflight token estimate
  └─ decides: send / compact / reject
        ↓
Frontend bridge executes provider SDK call
        ↓
Rust receives provider response
  ├─ persists promptTokens on last submitted input message
  ├─ stores assistant/tool results
  └─ continues workflow
```

---

## Compaction Flow

Compaction is a **before-send** overflow response.

If preflight says the next normal request exceeds `maxInputContext`:

1. Rust blocks the oversized normal request before send.
2. Rust selects a **resume-fit** ownership-safe `split_idx` (deepest split whose
   projected post-compact live prompt fits). Prompt-token checkpoints may seed
   candidates but must not alone commit a shallow oversized-tail split.
3. Rust emits a compaction request for that boundary (`to_id`).
4. Frontend executes the summary request and returns the result.
5. Rust stores the summary in `compact_contexts`.
6. Rust rebuilds the next request with the compact summary injected and the live
   tail after `to_id`.
7. Rust retries the normal completion request.

If no ownership-safe resume-fit split exists:

1. Rust does **not** send the oversized request.
2. Rust does **not** fabricate a lossy fallback.
3. Rust raises `INVALID_CONTEXT_STATE`.

This is intentional. Overflow without a safe resume-fit compaction boundary is an
invalid non-committing state.

### Resume-fit invariant (regression guard)

```text
chosen split must make:
  summary + retained_tail + system + tools
fit under effective_input_budget

compaction-input fitting must not change to_id
```

See the normative contract in
[`docs/specs/message-compaction.md`](../specs/message-compaction.md) §5.2.

---

## Prompt / Summary Semantics

### Preflight estimate vs actual submitted size

Do not confuse these:

- `conservative_prompt_tokens` = Rust preflight estimate
- `usage.promptTokens` = actual provider-submitted input size

If preflight blocks a request, that oversized normal request was never sent.

### Compact summary reinjection

When a valid compact summary exists, Rust injects it back into the logical message
stack as a synthetic compact-summary message and keeps only the live tail after
`to_id`.

The summary is persisted session state, not ephemeral UI state.

---

## Frontend Caveat

Frontend may still apply UX-level guards such as blocking blatantly oversized
first-input cases in ChatInput.

That does **not** change ownership:

- frontend convenience checks are advisory
- backend preflight remains authoritative

---

## Where To Read Next

Use these docs in this order:

1. **Normative contract**:
   [`docs/specs/message-compaction.md`](../specs/message-compaction.md)
2. **Implementation map**:
   `src-tauri/src/agent/llm/completion/`
3. **Persistence layer**:
   `src-tauri/src/repositories/compact_context_repository.rs`
   and `src-tauri/src/repositories/message_repository.rs`

---

## Bottom Line

For Agent V2, the safe mental model is:

- Rust owns context management.
- `message.promptTokens` is the persistent checkpoint truth (candidate seed).
- compact summaries are persisted session state.
- preflight compaction is a before-send gate.
- split selection is resume-fit first: make the next live prompt fit.
- missing ownership-safe resume-fit splits in overflow cases are invalid state,
  not silent fallback territory.
