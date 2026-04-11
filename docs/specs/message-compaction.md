# Message Compaction Contract

## 1. Scope

This document defines the **agreed normative contract** for Agent V2 message
compaction.

If the implementation differs from this document, the implementation should be
moved toward this contract rather than treating old behavior as authoritative.

It covers:

- context strategy separation
- compaction timing and workflow sequencing
- incremental summary folding
- prompt-cache layout expectations
- token semantics and limit interpretation
- compact-summary persistence and reinjection

---

## 2. Context Strategies

The system supports two context strategies:

| Strategy  | Behavior                                                         | Primary rule                      |
| --------- | ---------------------------------------------------------------- | --------------------------------- |
| `window`  | Sliding recent-message window only                               | No persisted summary state        |
| `compact` | Persisted summary plus recent tail with compaction orchestration | Summary state may be read/written |

### Contract

1. `window` mode remains a plain sliding-window strategy.
2. `compact` mode is the only strategy allowed to persist and reuse compacted
   summary state.
3. Compact-specific rules must not silently leak into `window` mode.

---

## 3. Ownership

Rust owns compaction orchestration and workflow control.

Frontend is only the provider bridge for:

- `llm:completion-request`
- `llm:compact-request`
- `llm:compact-state`

Frontend must not decide:

- when compaction should trigger
- what message slice should be compacted
- whether the next workflow step should be deferred
- how compact-summary state is persisted or reinjected
- whether a compact-mode request is safe to send

---

## 4. Core Model

Compaction is **incremental**, not full-prefix re-summarization.

The intended state model is:

```text
next_summary = compaction(prev_summary, message_delta_from_last_compaction)
```

Where:

- `prev_summary` is the persisted compact summary produced by the previous
  compaction
- `message_delta_from_last_compaction` is only the raw message delta accumulated
  since the previous compaction, not the full conversation history

### Contract

1. The summary is the persistent compacted state.
2. Newly accumulated messages are the delta.
3. Compaction folds delta into the previous summary.
4. Compaction should not repeatedly re-send already-compacted raw history when
   `prev_summary` already represents that history.
5. The first compaction in a session is the only case where there is no prior
   summary, so the compactable raw prefix becomes the initial delta baseline.

Equivalent interpretation:

```text
summary_state <- fold(summary_state, delta_messages)
```

This is the implemented model for compact mode.

---

## 5. Workflow Timing Contract

Compaction evaluation happens **once per completed assistant response**, not per
tool call.

Correct sequencing:

```text
assistant response completed
-> evaluate post-response compaction need
-> if compaction required, block next workflow step
-> run compaction
-> resume deferred next workflow step
```

### The deferred next workflow step may be

1. execute tool calls from the completed assistant response
2. request the next LLM turn
3. finalize the workflow

### Contract

1. A multi-tool assistant response is still a single response for compaction
   evaluation purposes.
2. Compaction must not be evaluated once per individual tool call in the same
   assistant response.
3. Tool execution may be deferred until compaction completes if the completed
   assistant response triggers compaction.

---

## 6. Sync Behavior Contract

Compaction is treated as synchronous from the perspective of workflow control,
even if the provider call itself is bridged asynchronously through the
frontend.

### Contract

While compaction is active:

1. compacting state must be visible in runtime/UI state
2. the next LLM turn must not start
3. deferred tool execution must not start
4. workflow completion must not be emitted if compaction has become the blocking
   next step

Completion of compaction is the gate that releases the deferred step.

### Rust-owned preflight gate

For compact mode, Rust owns the final pre-send hard gate.

1. Rust assembles the candidate request payload and computes the authoritative
   conservative preflight estimate.
2. If that estimate exceeds the send budget, Rust must not emit the completion
   request yet.
3. Rust should synchronously arm preflight compaction first, then retry with the
   rebuilt post-compaction payload.
4. Frontend may perform provider-specific prompt injection, but it is not the
   authority for compact-mode send/no-send decisions.

---

## 7. Effective Context Limit

Compact mode computes:

```text
safe_input_token_limit = min(max_input_context, model_max_limit)
```

### Contract

1. `safe_input_token_limit` is the configured request-budget target used by
   compact mode.
2. System prompt, session context, and tool schema belong to the same effective
   request budget.
3. Message selection should aim to keep assembled normal requests within that
   budget.
4. If actual submitted prompt size exceeds configured limit, that is a real
   oversize condition even if the provider still accepts the request because the
   provider hard max is larger.

---

## 8. Token Semantics

Token semantics must distinguish **provider-reported ground truth** from
**occupancy estimates used for control/UI**.

### Ground truth

- `promptTokens` is the provider-reported input token count for the actual
  submitted request
- `completionTokens` is the provider-reported output token count when provided

### Estimated / control values

- request-time message selection uses a prompt-anchored occupancy estimate
- post-response compaction trigger and UI gauge use Rust-emitted compaction
  pressure
- compaction pressure uses:

```text
reported promptTokens + conservative output estimate
```

### Contract

1. `promptTokens` is the source of truth for actual submitted input size.
2. request-time occupancy estimates are control heuristics, not provider-reported
   request size.
3. `promptTokens > configured limit` means the real submitted request exceeded
   the configured limit.
4. post-response compaction pressure is a conservative occupancy signal for
   trigger/UI purposes, not a claim about pure submitted input size.

---

## 9. Token Estimation Contract

Request-time occupancy estimation uses a prompt-anchored calibration model.

### Formula

Search backward for the latest assistant message with valid
`usage.promptTokens > 0`.

Then derive:

```text
ratio = promptTokens(anchor) / BPE(messages_before_anchor + sys + tools)
estimate = promptTokens(anchor) + BPE(messages[anchor_idx..]) * ratio
```

### Summary-aware anchor rule

`promptTokens(anchor)` already includes every stable input component that was sent
at that turn:

- compact-summary reinjection, if present
- system prompt
- session context
- tool schema
- the selected message tail up to the anchor turn

Therefore, once a grounded anchor exists, the estimator should treat those
stable inputs as already-accounted-for base state and primarily estimate only the
delta added after the anchor.

### Why promptTokens is the anchor

`promptTokens` is preferred over `totalTokens` because:

1. it measures pure input tokens
2. it grows monotonically with request history
3. it is less noisy than a total-based ratio that includes variable completion
   output

### Contract

1. The latest assistant turn with valid `usage.promptTokens > 0` is the preferred
   request-time anchor.
2. A compact-summary message **before** that grounded assistant does **not**
   invalidate the anchor by itself.
3. If the stable prompt layout is preserved, the estimator should reuse the
   anchor's reported input tokens as the base and estimate primarily the
   incremental delta after that anchor.
4. Full-BPE estimation is an exception path for turns with no grounded anchor,
   not the default strategy after compaction.
5. Rust preflight should apply a conservative upward bias to the estimated
   post-anchor delta before deciding whether the next request may be sent.

### Post-response trigger estimate

Post-response compaction decisions use:

```text
promptTokens + conservative_output_estimate
```

Where conservative output estimate is:

1. provider-reported `completionTokens`, if available
2. otherwise `totalTokens - promptTokens`, if available
3. otherwise local BPE fallback
4. plus a small upward safety bias

---

## 10. Prompt Cache Contract

Compaction should preserve the same stable prompt layout as normal requests as
much as possible.

Intended shape:

```text
next_summary = compaction(
  prev_summary,
  composed_layout_of_prompt,
  tool_schema,
  tool_call_disable,
  message_delta_from_last_compaction
)

next_output = llm_response(
  prev_summary,
  composed_layout_of_prompt,
  tool_schema,
  tool_call_enable,
  latest_context
)
```

### Contract

1. Compaction should reuse the stable prompt-cache prefix from normal requests.
2. Tool schema should stay present for cache-layout stability.
3. Compaction may disable tool use, but should not strip tool schema from the
   prompt layout merely because tool execution is disabled.
4. The compaction request and normal request need not be identical, but their
   stable prefix should remain aligned as much as possible.

Important clarification:

- this contract is about **stable prefix prompt cache reuse**
- it does **not** imply that compaction requests and normal response requests
  are byte-for-byte identical payloads

---

## 11. Compact Summary Persistence

Compaction state is session-scoped and persisted.

Persisted record fields:

- `session_id`
- `from_id`
- `to_id`
- `summary`
- `created_at`

Runtime session state may additionally track:

- in-flight compaction guard
- deferred workflow step
- completion blocking state
- compaction start time / observability
- last completion request layout for cache-preserving replay

### Contract

1. There is at most one active compact record per session.
2. Compact summary is the authoritative compressed history state for compact
   mode.
3. Updating compaction should replace the session's active compact record rather
   than creating nested summary chains.

---

## 12. Reinjection Contract

At request assembly time, if a compact record exists and its `to_id` still
matches the current message stack, Rust may synthesize:

```text
[compact-summary message] + [all messages after to_id]
```

### Contract

1. Reinjection is session-scoped.
2. Reinjection is valid only if the persisted range still matches the current
   stack.
3. Reinjection must not create nested summary-on-summary chains.
4. Summary reinjection is runtime context, not a user-authored persisted chat
   message.

---

## 13. Why Incremental Compaction Is Required

Even if prompt-cache prefix reuse works correctly, compaction input should still
stay away from:

```text
compaction(raw_compactable_prefix)
```

and remain at:

```text
compaction(prev_summary, delta_messages_since_last_compaction)
```

### Reasons

1. it bounds compaction payload growth
2. it avoids repeatedly sending already-compacted history
3. it better matches the intended summary-as-state, delta-as-input model
4. it remains compatible with stable prompt-cache prefix reuse

---

## 14. Frontend / UI Interpretation

The UI gauge is **Compaction Pressure**, not a raw-history meter.

It represents Rust-emitted post-response compaction pressure, not the total
persisted session history size and not a separate frontend estimate.

### Contract

1. UI should treat Rust-emitted compaction pressure as the SSOT for compact-mode
   occupancy display.
2. UI should not reinterpret compact-mode token accounting with a separate local
   estimator for control decisions.
3. A sharp drop after compaction is expected because the next request is rebuilt
   from compact-summary state plus recent tail.

---

## 15. Currently Confirmed Runtime Behavior

Already confirmed in logs:

1. post-response compaction evaluation runs after completed assistant responses
2. tool execution can be deferred until compaction completes
3. compact response is stored
4. deferred tool execution resumes after compaction

---

## 16. Alignment Status

Core implementation is now aligned with this contract:

1. compaction input is built from `prev_summary + delta_messages_since_last_compaction`
2. already-compacted raw history is not re-sent as full-prefix raw input
3. normal requests and compaction requests preserve stable prompt-layout inputs
   as much as practical while keeping compaction payload bounded
4. UI gauge uses Rust post-response compaction pressure as the displayed SSOT
