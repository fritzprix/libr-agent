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

## 1A. Governing Principles

The compact-mode contract is governed by five non-negotiable principles:

1. **SSOT principle**
   - Provider-reported `usage.promptTokens` is the source of truth for actual
     submitted input size.
   - Rust-emitted post-response compaction pressure is the source of truth for
     compact-mode occupancy display and trigger evaluation.
2. **Incremental estimation principle**
   - Request-time control estimates must stay anchored to the latest grounded
     `promptTokens` turn and estimate primarily the post-anchor delta.
   - Compaction itself is incremental summary folding, not full-prefix
     re-summarization.
3. **5% compaction-trigger margin principle**
   - Background post-response compaction should trigger once compact-mode
     occupancy exceeds 95% of the effective request budget.
   - This trigger threshold is distinct from the separate 5% conservative bias
     applied to estimated delta/output occupancy.
4. **Compaction-overflow-only drop principle**
   - Message dropping or truncation is allowed only when shrinking the
     compaction request payload itself so the compaction call can fit safely.
   - Normal compact-mode requests must not silently drop messages merely to make
     the next response request fit.
5. **Rust ownership principle**
   - All compact-mode routing, trigger, fit/no-fit, message slice, reinjection,
     and overflow decisions are made in Rust.
   - Frontend remains a provider bridge only.

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

Frontend may still perform provider-specific request assembly as part of that
bridge role, including:

- provider-specific prompt/context injection
- provider-specific tool normalization and request-body shaping
- provider-specific prompt-cache or cache-breakpoint shaping

But that bridge assembly must operate on Rust-owned contract inputs rather than
introducing frontend-owned compaction policy.

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
5. Rust's preflight contract must model the same logical request layout that the
   frontend bridge will submit, including compact-summary reinjection, retained
   tool schema, tool-use-disable policy, compaction-specific instruction input,
   and provider-visible session-context placement.
6. Frontend must not add compact-mode-only logical payload pieces that are
   invisible to Rust's fit/no-fit contract. Provider-specific serialization is
   allowed; frontend-owned logical reshaping is not.

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
3. In compact mode, Rust first assembles the next-response request from the
   current compact-summary state plus the live tail, then evaluates that
   candidate in the preflight gate. Rust must not silently drop or truncate
   messages just to force-send that next response request.
4. If the preflight gate determines that the assembled next-response request
   exceeds the budget, Rust must stay in the same compact flow and trigger
   preflight compaction first or raise an explicit context-limit error.
5. Message dropping or truncation may still be used when fitting the compaction
   input payload itself, because that step exists only to make the compaction
   request fit safely, not to reshape the normal next-response request.
6. If actual submitted prompt size exceeds configured limit, that is a real
   oversize condition even if the provider still accepts the request because the
   provider hard max is larger.

### Background compaction trigger threshold

Compact mode uses:

```text
compact_trigger_threshold = floor(safe_input_token_limit * 0.95)
```

### Contract

1. Background post-response compaction should trigger only after occupancy
   exceeds this 95% threshold.
2. Equality at the threshold is not itself a trigger condition; crossing above
   it is.
3. This 5% trigger margin is separate from the 5% conservative upward bias used
   inside token estimation helpers.

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
5. Normal compact-mode requests must not introduce request-side message drop
   noise into this SSOT signal; abrupt occupancy drops should come from
   compaction/reinjection state changes or from the actual grounded request
   history, not from pre-send compact-mode trimming.

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
6. The estimator's job is to decide send/no-send and compaction/no-compaction in
   Rust; it is not license for frontend-side request shaping.

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
5. Compaction should reuse the same provider-specific request-assembly path as
   normal requests wherever practical, so prompt/context injection, tool
   normalization, cache-key shaping, cache-breakpoint placement, and final
   request-body construction stay aligned by default.
6. Divergence between normal requests and compaction requests should be limited
   to compact-specific semantics:
   - `prev_summary + delta_messages_since_last_compaction` input shape
   - compaction instruction content
   - tool-use disabled while tool schema remains present
   - Rust-owned overflow reduction needed only to make the compaction request
     itself fit safely
7. A separate frontend-only compaction payload builder that bypasses or drifts
   from the normal vendor-specific assembly path is contrary to this contract,
   unless the same behavior is explicitly shared through a common assembly
   contract.

Important clarification:

- this contract is about **stable prefix prompt cache reuse**
- it does **not** imply that compaction requests and normal response requests
  are byte-for-byte identical payloads

---

## 10A. Compaction Overflow Recovery Contract

If a compaction request still overflows for **any** reason, the recovery goal
changes.

At that point, preserving prompt-cache alignment is no longer the top priority.
The top priority becomes: **reconstruct the most useful compactable state
possible, even if that causes a prompt-cache miss**.

Intended recovery shape:

```text
overflow_compaction_recovery(
  latest_real_user_request,
  prev_compaction_summary?,
  active_message_fifo_subset
)
```

### Essential recovery inputs

When overflow recovery is required, Rust should preserve these inputs in this
priority order:

1. **Latest real user request**
   - This is the highest-priority payload element.
   - It must refer to an actual user-authored request, not an internal synthetic
     user message.
   - The implementation must distinguish this using message source
     classification, not by `role == "user"` alone.
   - In particular, synthetic compact-mode/user-like messages such as
     `compact-summary`, `compaction-instruction`, `recovery`, and
     `session-context` must not be mistaken for the latest real user request.
2. **Previous compaction summary, if one exists**
   - If a prior compact summary is available, it should be preserved as the
     compressed history anchor whenever possible.
3. **Active message set as a partial FIFO subset**
   - The remaining live context may be reduced, but reduction should behave as a
     FIFO drop of older active messages so the newest active context survives as
     long as possible.

### Contract

1. Compaction overflow recovery is an **exception-only** path used after the
   normal cache-aligned compaction request still cannot fit.
2. A prompt-cache miss is acceptable in this path if that is what allows the
   system to preserve more useful recovery information.
3. Rust owns the recovery ordering and reduction policy.
4. The latest real user request must be preserved if it is at all possible to
   build any valid recovery payload.
5. If a previous compact summary exists, it should be preferred over re-sending
   older raw history.
6. Active messages may be reduced with FIFO semantics, but the recovery path
   should preserve the freshest active context rather than arbitrarily dropping
   recent turns first.
7. Tool schema may be degraded in this exception path when needed to make the
   recovery payload fit; for example, tool parameter schemas may be removed while
   retaining tool identity and any still-useful high-level tool visibility.
8. The system must not pretend this recovery payload is cache-aligned with the
   normal request shape once such degradation has occurred.
9. If even this ordered recovery contract cannot fit, the system should fail
   explicitly with a context-limit error rather than silently discarding the
   essential inputs above.

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

The tracked completion-request layout is a cache-preserving assembly contract,
not merely a loose metadata snapshot. It exists so compaction can reuse the same
stable provider-visible prompt layout as normal requests as much as practical.

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
4. A sharp drop caused by pre-send compact-mode request-side message dropping is
   not part of this contract.

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
5. compact-mode request overflow handling is Rust-owned and separate from
   compaction-payload overflow handling
