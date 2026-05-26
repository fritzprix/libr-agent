# Message Compaction Contract

## Quick Read

If you only need the contract in one minute, read this section first.

### The three layers

| Layer                               | What it answers                                                | Key rule                                                           |
| ----------------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------ |
| **Semantic compaction state**       | "What is being folded into the new summary?"                   | Normal path folds `prev_summary + full_delta`.                     |
| **Provider-visible request layout** | "What payload shape should the provider see?"                  | Compaction should reuse the same stable prefix as normal requests. |
| **Overflow recovery policy**        | "What may be reduced if compaction itself still does not fit?" | Split/reduction is allowed only here, not in the normal path.      |

### The one-line rule

```text
normal path = absorb full_delta into next_summary without silently trimming the normal request
```

### Normal path vs overflow recovery

| Question                                         | Normal compact path                      | Overflow recovery path                                |
| ------------------------------------------------ | ---------------------------------------- | ----------------------------------------------------- |
| May raw active windows be partially left behind? | **No**                                   | Yes, if required to fit recovery payload              |
| May prompt-cache alignment be broken?            | Should be preserved                      | May be sacrificed                                     |
| May tool schema be degraded?                     | **No**                                   | Yes, if needed                                        |
| May messages be dropped/truncated?               | **No** for the normal completion request | Yes, only to make the compaction/recovery request fit |

### Key terms

- **`prev_summary`**: the persisted compact summary from the previous compaction epoch
- **`full_delta`**: all uncompacted active windows after `prev_summary.to_id` in the current normal-path candidate request
- **stable prefix**: the provider-visible prompt/layout prefix that should stay aligned between normal requests and compaction requests
- **overflow recovery**: the exception path entered only when a cache-aligned compaction request still cannot fit

---

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
   - Rust-owned preflight fit/no-fit decisions are the source of truth for
     automatic compaction control in compact mode.
2. **Incremental estimation principle**
   - Request-time control estimates must stay anchored to the latest grounded
     `promptTokens` turn and estimate primarily the post-anchor delta.
   - Compaction itself is incremental summary folding, not full-prefix
     re-summarization.
3. **5% compaction-trigger margin principle**
   - Compact mode may use a 95% advisory threshold to proactively arm
     preflight compaction before the next request send.
   - This advisory threshold is distinct from the separate conservative bias
     applied to preflight occupancy estimation.
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

### 3A. Compaction request assembly boundary

The Rust/frontend split must be read narrowly:

1. **Rust owns logical compaction payload construction.**
   - Rust chooses the compacted message slice.
   - Rust injects any synthetic compact-summary anchor.
   - Rust generates and appends the compaction-instruction message.
   - Rust replays the parent request contract (`model`, `provider`, stable
     prompt inputs, tool set) needed to preserve request-layout alignment.
2. **Rust also owns provider-visible logical layout shaping before emit.**
   - If provider-specific session-context placement requires a synthetic
     message tail, Rust builds that logical message layout before the event is
     emitted.
   - Frontend must receive the already-shaped logical message list that Rust
     preflight fitted.
3. **Frontend owns only provider SDK / wire-format assembly.**
   - It may translate the Rust-provided logical layout into vendor-specific API
     fields, cache-breakpoint metadata, and transport-specific request bodies.
   - It must not invent or omit compact-mode message semantics on its own.

Equivalent split:

```text
Rust
= compaction policy + message slice + synthetic messages + logical layout contract

Frontend
= provider SDK adapter + final wire-format serialization
```

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
- in the normal path, that delta is the **full uncompacted live request slice**
  after `prev_summary.to_id` that would otherwise be carried into the next
  completion request

### Contract

1. The summary is the persistent compacted state.
2. Newly accumulated messages are the delta.
3. Compaction folds delta into the previous summary.
4. Compaction should not repeatedly re-send already-compacted raw history when
   `prev_summary` already represents that history.
5. The first compaction in a session is the only case where there is no prior
   summary, so the compactable raw prefix becomes the initial delta baseline.
6. In the normal compact-mode path, compaction input is `prev_summary +
full_delta`, where `full_delta` means all uncompacted active windows in the
   candidate next-request stack.
7. After a successful normal-path compaction, that pre-existing `full_delta`
   must be absorbed into `next_summary`; it must not remain outside the new
   summary as a residual live suffix in the next epoch.
8. Delta splitting, partial retention, FIFO reduction, or other message-slice
   shrinkage is an overflow-recovery behavior, not a normal compaction
   behavior.

Equivalent interpretation:

```text
summary_state <- fold(summary_state, delta_messages)
```

Important clarification:

- `prev_summary + delta_messages` describes the **semantic compaction state
  input**, not the full provider-visible request body by itself.
- The actual compaction provider request is assembled on top of the same
  provider-visible normal-request layout contract used for ordinary completion
  requests.
- That provider-visible layout may still include stable non-message inputs such
  as:
  - system prompt
  - session context placement
  - tool schema
  - provider-specific cache-key / cache-breakpoint shaping
- Therefore, the compaction contract has two layers that must not be confused:

```text
semantic compaction input
= prev_summary + full_delta

provider-visible compaction request
= normal_request_layout_base(stable prompt/context/tool/cache inputs)
  + compaction-specific overlay(prev_summary, full_delta, compaction_instruction, tool_use_disabled)
```

This is the normative model for compact mode.

### 4A. Compaction instruction contract

`compaction_instruction` is a **Rust-generated synthetic instruction turn**
addressed to the LLM summarizer.

### Contract

1. Its audience is the LLM performing the compaction call.
2. Its job is to define the required summary schema, compression rules,
   preservation hints, and tool-disable expectation for that compaction round.
3. It belongs to the **compaction overlay**, not the stable prefix.
4. It is represented as a synthetic **`user`** message, not a system prompt
   mutation.
5. Its message source must be classified as `compaction-instruction` so it is
   never confused with a real external user request.
6. Rust must generate this instruction before emitting `llm:compact-request`;
   frontend must treat it as already-authored logical input.
7. Because it is overlay content, it may differ between compaction rounds and
   is not itself a prompt-cache alignment anchor.

### 4B. Exact meaning of `full_delta`

`full_delta` means the **entire compactable normalized live slice** after the
previous summary boundary.

### Contract

1. Start from the current request candidate after Rust message normalization.
2. Exclude internal synthetic user messages used only for orchestration
   scaffolding.
3. If a previous compact record exists, begin immediately after `prev_summary.to_id`.
4. End at the current compaction split boundary.
5. Therefore `full_delta` may include:
   - assistant natural-language turns
   - assistant tool-call messages whose tool chain is resolved
   - tool result messages
   - the latest external user request, if it is already inside the compactable
     prefix
6. `full_delta` must not include the unresolved suffix behind the earliest
   still-open tool-call boundary.
7. In the common no-open-tool-chain case, `full_delta` extends to the end of the
   current normalized live stack.

---

## 5. Workflow Timing Contract

Automatic compaction evaluation happens **in preflight**, immediately before Rust
would send the next LLM completion request.

Correct sequencing:

```text
assistant response completed
-> execute tool calls / continue workflow / finalize normally
-> when the next LLM completion is about to be requested, run preflight fit check
-> if compaction required, block only that completion request
-> run compaction
-> retry the completion request with rebuilt compacted context
```

### Contract

1. Automatic compaction is a request-assembly responsibility, not a
   post-response workflow-orchestration responsibility.
2. Tool execution, pending-message continuation, and workflow finalization must
   not be deferred behind an automatic compaction roundtrip.
3. A multi-tool assistant response does not create a separate automatic
   compaction checkpoint after each tool call; the only automatic gate is the
   next completion preflight.

---

## 6. Sync Behavior Contract

Compaction is treated as synchronous from the perspective of workflow control,
even if the provider call itself is bridged asynchronously through the
frontend.

### Contract

While compaction is active:

1. compacting state must be visible in runtime/UI state
2. the next LLM turn must not start
3. the blocked unit of work is the pending completion request being prepared
4. tool execution and workflow completion are not retroactively re-blocked by
   automatic compaction once the response path has already advanced

Completion of compaction is the gate that releases the blocked completion
request.

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
7. In the normal compact-mode path, if the assembled request overflows, Rust
   must attempt compaction over `prev_summary + full_delta` first; it must not
   silently keep part of that pre-existing delta as a residual live suffix just
   to make the normal request fit.

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
7. Normal-path compaction must treat the current uncompacted active window set as
   the full delta to absorb, not as a pool that may be partially left behind in
   the next live stack.

### 7A. Compaction request budget and retry ladder

The compaction request itself is subject to the same effective context-budget
model. It is not exempt.

### Contract

1. The starting budget for the compaction request is the same
   `safe_input_token_limit`.
2. System prompt, replayed session context placement, tool schema, compact
   summary anchor, raw delta messages, and compaction instruction all consume
   that same budget.
3. If the cache-aligned compaction payload does not fit, Rust may enter the
   overflow-recovery ladder:
   - cache-aligned retry with stricter effective fit budget
   - overflow recovery payload reduction
   - degraded-tools recovery
4. If none of those paths fit, Rust must fail explicitly with a context-limit
   error rather than pretending compaction succeeded.

### 7B. Compaction frequency / duplicate suppression

The contract has **no time-based cooldown**. Duplicate suppression is structural,
not clock-based.

### Contract

1. A session may compact repeatedly across turns if the rebuilt request keeps
   exceeding the threshold.
2. Rust must not start a second independent compaction while one is already
   in-flight for the same session.
3. Rust may skip a new preflight compaction when the current live tail is the
   same tail that was already compacted successfully.
4. Therefore the frequency guard is:
   - in-flight reuse while compaction is running
   - same-tail suppression after settlement
   - not a wall-clock minimum interval

### Preflight advisory threshold

Compact mode uses:

```text
compact_trigger_threshold = floor(safe_input_token_limit * 0.95)
```

### Contract

1. Rust may use this threshold as an advisory preflight point for proactively
   arming compaction before the next completion request is emitted.
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
- preflight fit/no-fit and proactive compaction checks use Rust-owned
  conservative occupancy estimates
- the estimate uses:

```text
reported promptTokens + conservative output estimate
```

### Contract

1. `promptTokens` is the source of truth for actual submitted input size.
2. request-time occupancy estimates are control heuristics, not provider-reported
   request size.
3. `promptTokens > configured limit` means the real submitted request exceeded
   the configured limit.
4. conservative occupancy estimates are control signals for Rust preflight
   decisions, not claims about pure submitted input size.
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

### 9A. Anchor validity after reinjection changes

Compaction can change the live message layout around the old anchor. The rule is
simple: keep the anchor only while its prior grounded base still matches the new
layout assumption closely enough to remain incremental.

### Contract

1. A compact-summary message **before** the grounded assistant does not invalidate
   the anchor by itself.
2. If compaction inserts a new compact-summary message **after** the previously
   grounded anchor turn, the old anchor must not be trusted as if nothing
   changed.
3. After such a reinjection/layout shift, Rust may fall back to:
   - a newer grounded assistant anchor, if one exists
   - otherwise full-BPE estimation until a new grounded anchor is produced
4. Therefore, anchor reuse is conditional on layout continuity, not on message
   chronology alone.

### Preflight occupancy estimate

Preflight compaction decisions use:

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

To avoid ambiguity:

- `prev_summary + full_delta` is the semantic state being folded.
- It is **not** a claim that the provider request body contains only those two
  message components.
- Cache-preserving compaction still reuses the normal request assembly base,
  including stable prompt/context/tool inputs, and then applies only the minimal
  compaction-specific differences.

The easiest way to read this section is to separate **what is being folded**
from **what the provider sees**:

```text
semantic compaction input
= prev_summary + full_delta

provider-visible normal request
= stable_prefix(system_prompt, session_context, tool_schema, cache shaping)
  + normal_overlay(latest_context, tool_use_enabled)

provider-visible compaction request
= stable_prefix(system_prompt, session_context, tool_schema, cache shaping)
  + compaction_overlay(prev_summary, full_delta, compaction_instruction, tool_use_disabled)
```

So:

1. `tool_schema`, system prompt, and session context are **not** extra semantic
   fold inputs.
2. They **are** part of the stable provider-visible prefix.
3. Prompt-cache alignment is therefore a **provider-visible layout** concern, not
   a claim about semantic fold state.

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
   - `prev_summary + full_delta_messages_since_last_compaction` input shape in
     the normal path
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
- the compaction path should start from the same provider-visible normal request
  layout and then apply only the minimal compaction-specific semantic
  differences above

## 10A. Normal Active-Request Residual Contract

In the **normal compact-mode path**, the latest user/external request should
remain operationally available as **semantic residual state** inside the compact
summary, not as a permanently preserved raw request anchor in the live tail.

### Contract

1. Normal-path compaction should preserve the unresolved operative request in
   the `Active Request` summary section.
2. `Active Request` is semantic state, not a raw transcript dump; it should
   preserve intent, constraints, requested deliverables, and still-relevant
   qualifiers without forcing verbatim replay of the latest user message.
3. Latest external request block detection may still be used in Rust as a
   **distillation seed** for compaction hints and first-compaction coverage.
4. Normal-path compaction must not treat that raw request block as a hard
   anchoring contract that permanently pins the live tail boundary.
5. Incremental compaction may clear, supersede, or rewrite a previously
   summarized `Active Request` when later messages show that the request is
   resolved, replaced, or refined.
6. Durable outcomes from resolved requests should move into other stable summary
   sections rather than remaining as stale request bullets.
7. The current active windows submitted in the normal preflight candidate should
   be merged into the new summary state; they should not survive as a raw
   residual tail merely because they were active at the moment compaction was
   triggered.

---

## 10B. Compaction Overflow Recovery Contract

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

If this is the **first compaction in the session**, then `prev_compaction_summary`
is absent by definition. That is a normal first-compaction state, not a special
error.

In that case, the recovery shape should be read as:

```text
first_compaction_overflow_recovery(
  full_compactable_prefix,
  latest_real_user_request,
  active_message_fifo_subset
)
```

Where:

- `full_compactable_prefix` means the raw compactable prefix that the first
  compaction was attempting to summarize before any prior summary existed
- this is still an **overflow-recovery** path, not a successful normal-path
  compaction
- the absence of `prev_compaction_summary` does not weaken the requirement to
  preserve the most useful operative context possible

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
6. If this is the first compaction and no previous compact summary exists, the
   recovery baseline is the first compaction's raw compactable prefix rather than
   a prior summary anchor.
7. First-compaction overflow recovery may therefore operate on a reduced form of
   that raw compactable prefix while still preserving the latest real user
   request and the freshest active context.
8. Active messages may be reduced with FIFO semantics, but the recovery path
   should preserve the freshest active context rather than arbitrarily dropping
   recent turns first.
9. Tool schema may be degraded in this exception path when needed to make the
   recovery payload fit; for example, tool parameter schemas may be removed while
   retaining tool identity and any still-useful high-level tool visibility.
10. The system must not pretend this recovery payload is cache-aligned with the
    normal request shape once such degradation has occurred.
11. If even this ordered recovery contract cannot fit, the system should fail
    explicitly with a context-limit error rather than silently discarding the
    essential inputs above.
12. Any split/reduction of the normal full delta belongs to this exception path,
    not to normal-path compaction.

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

### 11A. Boundary ID contract (`from_id`, `to_id`)

`from_id` and `to_id` are **message identity boundaries**, not ordinal indexes.

### Contract

1. `from_id` and `to_id` must store `message.id` string values from the compacted
   persisted message range.
2. They do **not** store:
   - array positions
   - FIFO ordinals
   - "nth message in stack" style counters
3. `to_id` means: "this compact record covers messages through the persisted
   message whose `id` equals `to_id`."
4. Reinjection validity is checked by matching `to_id` back against the current
   message stack, not by recomputing a numeric split index.
5. The identity contract assumed here is:
   - persisted messages use stable message IDs
   - persisted message IDs are unique at the storage layer
6. This contract does **not** require every synthetic runtime-only message ID to
   be globally UUID-shaped.
7. Therefore, compaction boundary correctness depends on matching persisted
   message identity, not on any assumption that all message IDs across all
   runtime scaffolding are globally random UUIDs.

### 11B. Compact-summary message contract

The persisted compact record is storage state. The reinjected compact-summary
message is the runtime message-form projection of that state.

### Contract

1. The reinjected compact-summary message must use role **`assistant`**.
2. Its source classification must be **`compact-summary`**.
3. It is synthetic runtime context, not a user-authored transcript turn.
4. Its content should contain the compact summary text and may include a bounded
   recent tool snapshot if the implementation uses one.
5. It participates in the provider-visible request payload and therefore affects
   prompt tokens and cache alignment like any other injected message.
6. It must remain distinguishable from normal assistant turns so anchor logic,
   recovery logic, and UI logic do not misclassify it.

### 11C. Message source classification contract

Role alone is not enough. The compaction contract depends on explicit message
source classification.

### Contract

1. Every synthetic orchestration message should carry a stable source label.
2. At minimum, the contract recognizes these synthetic classes:
   - `compact-summary`
   - `compaction-instruction`
   - `session-context`
   - `recovery`
3. External user requests are classified by source policy, not by `role == "user"`
   alone.
4. Internal synthetic user messages must never be treated as the latest real user
   request during overflow recovery.
5. Unknown future source values should degrade safely rather than breaking
   deserialization.

### 11D. In-flight compaction guard

The compaction guard is the session-scoped runtime state that prevents duplicate
or contradictory compaction work.

### Contract

1. It prevents concurrent independent compaction requests for the same session.
2. It marks whether the in-flight compaction is:
   - manual only, or
   - preflight-blocking and must resume the blocked completion afterward
3. It stores enough runtime state to support:
   - same-tail suppression
   - overflow-recovery retries
   - completion resume after success
4. On successful compaction, the guard settles back to idle and may trigger
   completion resume.
5. On failure, the guard must be cleared so the session does not remain stuck in
   a fake in-flight state.

### 11E. Compaction failure handling

Compaction failure is not one thing. Budget-related fit failures and hard
execution failures must be treated differently.

### Contract

1. If the compaction call fails for budget-related reasons, Rust may advance the
   overflow-recovery ladder and retry compaction.
2. If compaction ultimately succeeds, Rust stores the new compact record and, for
   preflight compaction, resumes the blocked completion request.
3. If compaction fails with a non-recoverable error:
   - in-flight compaction state must be cleared
   - failure state must be emitted to the UI
   - a blocking preflight compaction must fail the waiting workflow explicitly
4. The system must not silently continue as if the blocked completion had been
   compacted when it was not.

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
5. After a successful normal-path compaction, the messages after `to_id` should
   ordinarily represent only turns created after that compaction completed, not
   pre-existing active windows that were omitted from the same compaction epoch.

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

Frontend is not the authority for automatic compaction decisions.

### Contract

1. UI should reflect Rust-emitted compact-state events for actual in-flight
   compaction.
2. UI must not implement its own automatic compaction trigger logic.
3. Any future occupancy gauge is optional product UI, not part of the automatic
   compaction contract.
4. UI token displays must not override Rust preflight fit/no-fit decisions.

---

## 15. Currently Confirmed Runtime Behavior

Already confirmed in logs:

1. automatic compaction is armed from request preflight, not from a completed
   response tail
2. compact responses are stored and reinjected through compact-summary state
3. preflight compaction blocks only the pending completion request
4. workflow progression no longer relies on deferred post-response resume steps

---

## 16. Alignment Evaluation Rule

Implementations should be judged against this contract using the following
rules:

1. Normal-path compaction is aligned only if it absorbs `prev_summary +
full_delta_messages_since_last_compaction` into `next_summary`.
2. An implementation is not aligned if a successful normal-path compaction
   leaves pre-existing active windows outside the new summary as a residual live
   suffix.
3. An implementation is not aligned if it silently trims or drops messages from
   the normal compact-mode completion request merely to force-send that request.
4. Delta splitting, FIFO reduction, tool-schema degradation, and prompt-cache
   misalignment are aligned only inside the explicit overflow-recovery path.
5. Prompt-cache alignment is considered preserved only when compaction reuses
   the same normal request assembly contract by default and limits divergence to
   the compaction-specific semantics defined above.
