# Message Compaction Spec and Contract

## 1. Scope

This document defines the **current Rust-side contract** for Agent V2 message
context management on `dev/0.7.x`.

It covers:

- context strategy split (`window` vs `compact`)
- request-time message stack assembly
- compaction trigger and recovery behavior
- compact-summary persistence and reinjection
- context selection rules and token budgeting
- provider/tool-chain safety guarantees
- scheduled-task behavior under the same compaction contract

This document is normative for the current implementation. If the code changes,
this file should be updated with it.

---

## 2. Context Strategies

The system supports two context strategies:

| Strategy | Behavior | Primary path |
| --- | --- | --- |
| `window` | Sliding window over recent messages only. No summary persistence. | `select_recent_messages_fifo()` |
| `compact` | Async compaction with persisted summary + recent tail. | `request_llm_completion()` + `select_messages_within_context()` + compaction pipeline |

### Contract

1. **`window` mode** remains a plain sliding-window strategy.
2. **`compact` mode** is the only strategy that may read/write compact-summary
   state.
3. The two strategies must remain behaviorally separated. A compact-specific
   rule must not silently leak into window mode.

---

## 3. Ownership and Main Code Paths

Rust owns compaction orchestration.

Primary files:

- `src-tauri/src/agent/llm/completion/request.rs`
- `src-tauri/src/agent/llm/completion/compaction.rs`
- `src-tauri/src/agent/llm/context_selector.rs`
- `src-tauri/src/agent/llm/token_utils.rs`
- `src-tauri/src/agent/llm/response.rs`
- `src-tauri/src/agent/state.rs`

Frontend acts as the provider bridge for:

- `llm:completion-request`
- `llm:compact-request`

The frontend does **not** decide compaction strategy, split points, token
selection, summary reinjection, or scheduled-task recovery semantics.

---

## 4. Request-Time Flow in Compact Mode

`request_llm_completion()` is the source of truth.

High-level flow:

1. validate session state
2. load prompt and context-management settings
3. drain pending user messages into the in-memory cache
4. filter recovery-only messages from normal prompt context
5. resolve references and merge recovery-produced trailing user runs if needed
6. re-inject a persisted compact summary if the saved range is still valid
7. compute token state and overflow preflight
8. optionally trigger preflight compaction and pause the LLM request
9. optionally trigger background compaction when threshold is crossed
10. select final messages within the safe limit
11. emit `llm:completion-request`

### Contract

Compaction mode must always build the final LLM stack from:

```text
[optional compact-summary] + [selected recent tail]
```

The summary is synthetic runtime context, not a user-authored persisted message.

---

## 5. Effective Context Limit

Compact mode computes:

```text
safe_input_token_limit = min(max_input_context, model_max_limit)
```

This value is the hard limit for request-side context preparation.

### Contract

1. System prompt tokens and tool-definition tokens are budgeted before message
   selection.
2. Message selection must never intentionally exceed
   `safe_input_token_limit`.
3. A context-limit error means compaction/recent selection could not produce a
   valid stack within that limit.

---

## 6. Token Estimation Contract

Token estimation uses `calculate_grounded_total_tokens()`.

Behavior:

1. If a recent assistant message has provider-reported `usage.totalTokens`,
   that value is used as the grounded anchor.
2. Messages after the grounded point are incrementally estimated via BPE.
3. If no grounded anchor exists, fallback is full BPE across the candidate
   stack.

### Contract

1. Grounded usage is preferred over pure local estimation when available.
2. Tool definitions and prompt text are included in total request budgeting.
3. Selection decisions must be based on the same budgeting model used by the
   request path, not on ad hoc UI estimates.

---

## 7. Compact Summary Persistence and Reinjection

Compaction state is session-scoped and persisted.

In-memory session fields:

- `compact_context`
- `compact_in_flight`
- `last_compacted_tail_id`
- `awaiting_compact_completion`
- `compact_started_at_ms`
- `last_completion_request`

Persisted record fields:

- `session_id`
- `from_id`
- `to_id`
- `summary`
- `created_at`

### Reinjection behavior

At request start, if a compact record exists and `to_id` is still present in
the current stack:

1. Rust creates a synthetic user message:
   - `id = compact-summary-{session_id}`
   - `source = "compact-summary"`
2. Rust replaces the compacted prefix with:

```text
[compact-summary message] + [all messages after to_id]
```

If `to_id` is no longer present, the cached compact record is treated as stale
and invalidated.

### Contract

1. There is at most one active compact record per session.
2. Reinjection is valid only if the saved `to_id` still matches the current
   stack.
3. Summary reinjection must never create nested summary-on-summary chains.

---

## 8. Compaction Trigger Contract

Compact mode has two trigger classes.

### 8.1 Background compaction

When token usage crosses the compact threshold, Rust may launch async
background compaction.

### 8.2 Preflight compaction

When a request is already over the safe limit, Rust may trigger preflight
compaction immediately and pause the LLM request until the compaction result is
available.

### Guards

The system prevents duplicate or pointless compaction with:

1. `compact_in_flight` — only one in-flight compaction per session
2. `last_compacted_tail_id` — prevents re-compacting the same unchanged tail

### Contract

1. Two overlapping compactions for the same session must not run concurrently.
2. Preflight compaction is allowed to pause a request.
3. If no new tail exists since the last compaction, the same-tail guard may
   skip another compaction attempt.

---

## 9. Context Selection Contract in Compact Mode

Compact mode uses `select_messages_within_context()`.

Selection behavior:

1. messages may be batched to keep tool call groups coherent
2. incomplete/orphan tool chains are removed for provider families that require
   strict tool-call integrity
3. messages are selected newest-to-oldest until budget is exhausted
4. final selection order is restored to chronological order

### 9.1 First user message pinning

**Current contract on `dev/0.7.x`:**

- `select_messages_within_context()` supports an explicit
  `SelectionOptions.pin_first_user_message` flag
- the default is `true`
- **compact mode passes `pin_first_user_message: false`**
- `window` mode does not use this selector and is unaffected

### What "pinning" means

When pinning is enabled, the selector:

1. reserves budget for the first user message
2. excludes that message from reverse tail scanning
3. may prepend it back to the final result
4. may merge it with the first selected user message if both are user turns

When pinning is disabled, none of that special handling applies. The first user
message is just another candidate in the normal reverse scan.

### Contract

1. **Compact mode must not pin the first user message.**
2. **Window mode behavior must remain unchanged.**
3. Disabling pinning must also disable all pinning-derived budget reservation,
   skip logic, max-message adjustment, and prepend/merge behavior.

This is important: turning off pinning is **not** just a presentation tweak. It
changes token reservation and final message assembly.

---

## 10. Tool-Chain Safety Contract

For providers with strict tool sequencing requirements, compact-mode selection
must not leave broken tool chains in the final prompt.

Current protected provider family:

- `anthropic`
- `gemini`
- `openai`
- `openrouter`
- `groq`

Rules:

1. orphan tool results are removed
2. unresolved assistant tool-call messages are sanitized or dropped from the
   unstable suffix
3. the selected prompt must not contain a dangling tool chain that the provider
   would reject

### Contract

Compaction must preserve provider-valid tool-call structure even when old
history is summarized away.

---

## 11. Error Contract

Context-limit errors in compact mode are explicit runtime errors, not silent
fallbacks.

Two main cases:

1. **Latest input too large**
   - newest non-compactable tail alone exceeds the safe limit
2. **Context limit exceeded after compaction recovery attempt**
   - even after compaction handling and selection, request still cannot fit

### Contract

1. Compact mode must fail loudly when a valid stack cannot be assembled.
2. Errors must remain actionable and distinguish between
   "newest tail is too large" vs "overall context still does not fit".
3. Failure must not silently switch to a different context strategy.

---

## 12. Scheduled Task Contract

Scheduled tasks use the same session context-management rules as normal agent
sessions.

`src-tauri/src/scheduled/runner.rs` currently:

1. resolves the task session
2. resumes or creates it
3. injects the scheduled message
4. lets the normal workflow/request path handle compact or window context logic

### Contract

1. Scheduled sessions do **not** have a special context-limit rotation path on
   `dev/0.7.x`.
2. Scheduled sessions must rely on the same compact/window behavior as normal
   sessions.
3. Scheduled execution must not create a fresh session just because compact-mode
   context handling failed.

This keeps scheduled behavior aligned with regular sessions instead of adding a
special recovery rule.

---

## 13. Behavior Changes Introduced by the Current Contract

The most important current branch-level behavior is:

### Compact mode no longer preserves the oldest user turn by force

Implications:

1. more budget is available for the recent tail
2. compaction summary becomes the long-term history carrier
3. recent tool chains and recent user/assistant turns are favored over the
   oldest raw user message

### Expected tradeoff

If early user intent was not sufficiently captured in the compact summary, it
may disappear sooner from raw prompt context. That is an intentional tradeoff in
exchange for a cleaner, strictly recent-tail compaction model.

---

## 14. Required Invariants

Any future refactor must preserve these invariants unless this spec is updated:

1. `window` and `compact` remain distinct strategies.
2. Compact-summary reinjection is session-scoped and validated by `to_id`.
3. Compact mode does not pin the first user message.
4. Window mode behavior is unchanged by compact-mode selector rules.
5. Provider-sensitive tool-chain cleanup remains intact.
6. Scheduled tasks do not have a special rotation/recreate escape hatch.
7. Context-limit failures are explicit and actionable, never silent.

---

## 15. Files That Define This Contract

Primary implementation:

- `src-tauri/src/agent/llm/completion/request.rs`
- `src-tauri/src/agent/llm/completion/compaction.rs`
- `src-tauri/src/agent/llm/context_selector.rs`
- `src-tauri/src/agent/llm/token_utils.rs`
- `src-tauri/src/agent/llm/response.rs`
- `src-tauri/src/agent/state.rs`
- `src-tauri/src/scheduled/runner.rs`

Primary tests:

- `src-tauri/tests/llm_context_tests.rs`

If behavior changes in these files, this document should be updated in the same
change.
