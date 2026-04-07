# Compaction Overflow Analysis (2026-04-07)

## Summary

This note explains a context-management failure where the agent exceeded the
effective context limit even though compaction is supposed to trigger when the
conversation reaches roughly 90% of the configured context window.

The short version:

1. Rust decided whether to compact based on a **grounded token estimate**.
2. That grounded estimate could become **stale** and undercount the current
   request payload.
3. The frontend later built the **provider-ready payload** and rejected it with
   `prepared_payload_too_large`.
4. In the captured incident, a preflight compaction was triggered, but the
   compaction LLM call failed with a transport/connection error, so no compact
   summary was injected and retries kept hitting the same oversized payload.

## Symptom

Closest available error evidence:

- `error_2026-04-08.txt`
- `log_2026-04-08.txt`

Key error:

```text
Prepared payload exceeds the effective context limit (52354 > 49152)
```

Captured frontend details:

```text
projectedPayloadTokens=49896
safetyMargin=2458
effectiveContextLimit=49152
```

This means the frontend believed the final provider-shaped request would cost
`49896 + 2458 = 52354`, which is above the allowed `49152`.

## Important Non-Fix: Do Not Remove Tools From Compaction

It is tempting to strip `availableTools` from compaction requests because
compaction calls run with tool use disabled. That is the wrong fix.

Compaction intentionally keeps the parent request shape, including
`availableTools`, to preserve provider prompt-cache behavior and request-layout
stability.

Two relevant code paths:

- `src/lib/ai-service/openai.ts`
  - keeps `tools` in the request
  - sets `tool_choice: 'none'` when `disableToolUse` is true
- `src/lib/ai-service/gemini/service.ts`
  - keeps tool declarations in the payload
  - disables function calling with `FunctionCallingConfigMode.NONE`
  - uses the stable prompt/tool layout for cache decisions

So the fix should target **budget accounting**, not cache-shape preservation.

## Why the 90% Trigger Was Not Enough

The 90% threshold is only as good as the estimate used to measure the current
payload.

### Rust-side trigger path

Rust used `calculate_grounded_total_tokens(...)` in multiple places:

- context selection calibration
- preflight overflow handling
- background compaction threshold checks

Relevant files:

- `src-tauri/src/agent/llm/token_utils.rs`
- `src-tauri/src/agent/llm/context_selector.rs`
- `src-tauri/src/agent/llm/completion/request.rs`

### Frontend final guard path

The frontend later computed the provider-ready payload size in
`src/context/llm/useExecuteCompletion.ts`:

1. provider-specific `prepareContextInjection(...)`
2. `estimatePayloadTokens(...)`
3. `calculateContextSafetyMargin(...)`
4. hard reject if payload + margin exceeds the effective limit

This check is the one that produced `prepared_payload_too_large`.

## Root Cause

### 1. Grounded usage could become stale

Before the patch, `calculate_grounded_total_tokens(...)` preferred the most
recent assistant message with API-reported `usage.totalTokens`:

```rust
grounded_estimate = previous_assistant_usage_total + tokens_after_that_message
```

That is useful when the grounded value still reflects the current request
layout, but it breaks when the current request has materially changed since the
grounded assistant turn.

Examples of drift:

- current system prompt grew
- session context changed
- tool payload changed
- current local BPE estimate is already larger than the old grounded base

When that happened, Rust could think the current request was still below the
background compaction threshold even though the actual provider-ready payload was
already close to, or beyond, the hard limit.

### 2. Background compaction used that stale estimate

`should_trigger_background_compaction(...)` only sees the token count it is
given. If `current_tokens` is undercounted, compaction does not fire early
enough.

That is why the system can miss the intended “compact at 90%” behavior: the
decision is not based on the final request payload, but on an estimate that may
already be wrong.

### 3. Frontend caught the real payload later

The frontend is closer to the actual provider request shape, so it can reject a
payload that Rust previously allowed:

```text
Rust: "looks safe enough"
Frontend: "nope, this exact payload is too large"
```

That mismatch is the core design gap.

### 4. Failed preflight compaction made the symptom worse

From the logs, a preflight compaction was eventually triggered, but the compact
LLM call failed with a connection error. Because no compact summary was stored,
the system remained stuck with the same oversized history and retries kept
failing.

That failure amplified the incident, but it was not the primary accounting bug.

## Patch

### Change

File changed:

- `src-tauri/src/agent/llm/token_utils.rs`

The fix makes grounded estimation conservative:

```rust
final_estimate = max(grounded_estimate, full_bpe_estimate)
```

Where:

- `grounded_estimate` = previous grounded API total + post-grounding increments
- `full_bpe_estimate` = current full local estimate over all messages plus
  system/tool overhead

### Why this works

If grounded usage is still trustworthy and higher than the local estimate, we
keep it.

If grounded usage is stale and lower than the current full-request estimate, we
refuse to undercount.

This closes the hole where compaction thresholds and preflight selection could
make decisions using an estimate that was already smaller than the current
request.

## Regression Coverage

Tests added/updated in:

- `src-tauri/tests/llm_context_tests.rs`

Coverage now includes:

1. normal grounded estimation
2. compaction-summary fallback behavior
3. stale grounded usage not being allowed to undercut the current full BPE
   estimate

## Validation

Validated with:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test llm_context_tests
pnpm refactor:validate
```

Both passed after the patch.

## Practical Debugging Checklist

When someone hits a similar overflow again, check these in order:

1. **Frontend error details**
   - Look for `prepared_payload_too_large`
   - Capture `projectedPayloadTokens`, `safetyMargin`, and `effectiveContextLimit`

2. **Compaction chronology**
   - Was background compaction triggered before failure?
   - Was preflight compaction triggered only after the payload was already too
     large?
   - Did the compact request itself fail?

3. **Grounded vs full estimate mismatch**
   - Is Rust relying on an older assistant `usage.totalTokens`?
   - Has session context or tool payload grown since that grounded turn?

4. **Do not “fix” it by removing tools from compaction**
   - That breaks prompt-cache alignment and is not the root cause

## Follow-up Ideas

This patch fixes the immediate undercount, but there is still room to tighten
the design:

1. thread more provider-ready budgeting context deeper into preflight decisions
2. add stronger handling for repeated retries after compaction transport failure
3. consider persisting richer request-shape metadata alongside usage so grounded
   estimates can be invalidated explicitly instead of inferred conservatively

## Reference Files

- `src-tauri/src/agent/llm/token_utils.rs`
- `src-tauri/src/agent/llm/context_selector.rs`
- `src-tauri/src/agent/llm/completion/request.rs`
- `src/context/llm/useExecuteCompletion.ts`
- `src/context/llm/useLLMListener.ts`
- `src/lib/ai-service/openai.ts`
- `src/lib/ai-service/gemini/service.ts`
- `error_2026-04-08.txt`
- `log_2026-04-08.txt`
