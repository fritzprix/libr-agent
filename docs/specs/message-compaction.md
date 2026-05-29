# Message Compaction Contract

## Quick Read

This document defines the current compaction contract for Agent V2.

The short version:

1. `message.promptTokens` is the persistent checkpoint truth.
2. The value belongs to the **last submitted input message** of a successful request.
3. Rust preflight uses the latest checkpoint plus estimated delta to decide whether
   the next request fits.
4. If the request does not fit, Rust compacts before send.
5. If the request does not fit and there is no usable checkpoint anchor, the state
   is invalid and the backend must reject without mutating history.

The old mental model around `safeCheckpointId` is obsolete. Context-fit truth must
survive restart and later `maxInputContext` changes, so the checkpoint must live on
messages themselves.

---

## 1. Scope

This spec covers:

- persisted token checkpoints
- request-time preflight behavior
- compaction trigger behavior
- invalid-state handling
- compact summary persistence and reinjection
- UI-facing condensed-count meaning

This spec does **not** define provider-specific SDK request formatting.

---

## 2. Core Contract

### 2.1 Source of truth

There are two different truths and they must not be confused:

1. **Provider-reported truth**
   - `usage.promptTokens` is the actual submitted input size for a completed
     request.
2. **Rust preflight truth**
   - `conservative_prompt_tokens` is a Rust-side estimate used to decide whether
     the next request is safe to send.

Important consequence:

```text
preflight estimate != actual submitted size
```

If Rust preflight blocks a request, that blocked request was never submitted to the
provider.

### 2.2 Persistent checkpoint rule

After a successful completion request, the backend must persist the provider's
`usage.promptTokens` onto the **last submitted input message** for that request.

That field is stored as:

```text
message.promptTokens?: number | null
```

This value means:

```text
"total input tokens processed when the conversation state included this message as
the request tail"
```

It is **not**:

- a per-message token size
- a delta to sum across turns
- an assistant-message metric

### 2.3 Nullability rule

`message.promptTokens` may be `null` for older history, failed attempts, imported
data, or any message that has never served as the final submitted input checkpoint.

Therefore:

```text
null means "not checkpointed yet", not "zero tokens"
```

Any algorithm that uses `message.promptTokens` must explicitly handle `null`.

---

## 3. Data Model

### 3.1 Message

Relevant fields:

```ts
type Message = {
  id: string;
  role: 'system' | 'user' | 'assistant' | 'tool';
  promptTokens?: number | null;
};
```

### 3.2 Runtime session state

The runtime tracks:

```text
last_submitted_input_message_id
```

This exists only to bridge the gap between:

1. request emission time
2. provider response time

Flow:

1. request orchestration decides which input message is the last submitted input
2. runtime records its id
3. response handling receives provider `usage.promptTokens`
4. backend writes that value back onto the recorded message

### 3.3 Compact context

Compaction persists a summary record anchored by `to_id`.

The summary means:

```text
"all compacted live history through to_id is now represented by this summary"
```

The old `from_id`-centric interpretation is intentionally gone from the contract.

---

## 4. Request-Time Preflight Rules

Before a normal completion request is sent, Rust computes token-fit metrics against
the configured `maxInputContext`.

### 4.1 Safe limit

`maxInputContext` from settings is the authoritative configured input budget.

### 4.2 Fit decision

Rust estimates the next request size conservatively.

If the estimate is within the safe limit:

```text
send normal completion request
```

If the estimate exceeds the safe limit:

```text
do not send the normal completion request
```

Then Rust must choose one of two paths:

1. compaction is possible -> trigger compaction first
2. compaction is impossible -> reject as invalid non-committing state

### 4.3 Why checkpoints matter

The checkpoint is needed because `maxInputContext` may shrink later.

Example:

1. a request previously succeeded under a large context window
2. the user lowers `maxInputContext`
3. old `safeCheckpointId` assumptions become stale
4. persisted `message.promptTokens` still reflects real previously observed input
   occupancy

That is why checkpoint truth must be stored on messages and persisted in the DB.

---

## 5. Compaction Trigger Rules

### 5.1 Normal policy

Compaction is a **before-send** operation.

If preflight says the next request does not fit:

1. Rust blocks the normal request
2. Rust computes the compactable range
3. Rust sends a compaction request
4. Rust stores the resulting summary
5. Rust rebuilds the request with the compact summary injected
6. Rust retries the normal completion request

### 5.2 Range selection principle

The compactable cutoff is anchored by persisted prompt-token checkpoints, not by the
old `split_idx` mental model alone.

The point of the checkpoint is:

```text
find the latest proven-fit anchor below the current request tail, then compact the
older live prefix into summary state
```

### 5.3 What the new summary absorbs

On successful compaction:

```text
new_summary = fold(prev_summary, full_compacted_delta)
```

Where `full_compacted_delta` is the uncompacted live range being folded in that
round.

Normal compaction must not silently leave half of that selected delta outside the
summary.

---

## 6. Invalid State Rule

If Rust preflight determines the request does not fit, but there is no usable
prompt-token checkpoint to anchor a safe compaction target, that state is invalid.

Required behavior:

1. reject the request in backend
2. do not commit a misleading summary
3. do not mutate message history as if compaction succeeded
4. surface an explicit invalid-context error

This is a non-committing failure, not a silent fallback.

---

## 7. Frontend Role

Frontend is not the authority for compaction decisions.

Rust owns:

- token-fit judgment
- compaction triggering
- compactable range selection
- invalid-state rejection
- summary persistence and reinjection

Frontend may provide a lightweight UX guard for obviously bad first-input cases, but
that guard is only a convenience layer.

Rules:

1. ChatInput may reject a blatantly oversized first input early.
2. That early rejection must stay conservative and UX-oriented.
3. Backend must still re-validate and remain authoritative.

---

## 8. Summary Bubble Semantics

The summary bubble field:

```text
xxx messages condensed
```

means:

```text
the number of live messages absorbed by the compacted delta represented by this
summary event
```

It does **not** mean:

- total messages in the whole conversation
- total messages ever seen by the session
- provider-visible request message count

So the UI count must reflect the selected compacted delta only.

---

## 9. Canonical Pseudocode

The following pseudocode is the intended behavior model.

```ts
type Message = {
  id: string;
  role: 'user' | 'assistant' | 'tool';
  promptTokens?: number | null;
  usage?: {
    completionTokens?: number | null;
  } | null;
  source?: 'external_request' | 'internal' | 'tool' | null;
};

type CompactSummary = {
  toId: string;
  summary: string;
  condensedCount: number;
} | null;

const OUTPUT_RESERVE_FALLBACK_CAP = 8192;

function latestCheckpoint(messages: Message[]): Message | null {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    if (messages[i].promptTokens != null) {
      return messages[i];
    }
  }
  return null;
}

function estimateNextPromptTokens(args: {
  systemTokens: number;
  toolTokens: number;
  serviceContextTokens: number;
  liveMessages: Message[];
  compactSummary: CompactSummary;
}): number {
  // Exact estimator details are implementation-owned.
  // Contractually this is a conservative Rust preflight estimate.
  return conservativeEstimate(args);
}

function deriveMeasuredOutputTokensReserve(args: {
  liveMessages: Message[];
  configuredMaxOutputTokens?: number | null;
}): number {
  const latestExternalRequestIndex = findLastIndex(
    args.liveMessages,
    (message) => message.source === 'external_request',
  );

  const latestExternalRequestCycleMax =
    latestExternalRequestIndex == null
      ? null
      : max(
          args.liveMessages
            .slice(latestExternalRequestIndex)
            .filter((message) => message.role === 'assistant')
            .map((message) => message.usage?.completionTokens ?? null),
        );

  const latestObservedAssistantCompletionTokens = lastValue(
    args.liveMessages,
    (message) =>
      message.role === 'assistant' ? message.usage?.completionTokens ?? null : null,
  );

  const configuredReserve = Math.min(
    args.configuredMaxOutputTokens ?? 0,
    OUTPUT_RESERVE_FALLBACK_CAP,
  );

  return Math.min(
    latestExternalRequestCycleMax ??
      latestObservedAssistantCompletionTokens ??
      configuredReserve,
    OUTPUT_RESERVE_FALLBACK_CAP,
  );
}

function calculateEffectiveInputBudget(args: {
  safeInputTokenLimit: number;
  measuredOutputTokensReserve: number;
}): number {
  return Math.max(
    0,
    args.safeInputTokenLimit - args.measuredOutputTokensReserve,
  );
}

function findCompactTarget(args: {
  liveMessages: Message[];
  effectiveInputBudget: number;
}): { toId: string; condensedCount: number } | null {
  const checkpoint = latestCheckpoint(args.liveMessages);
  if (!checkpoint || checkpoint.promptTokens == null) {
    return null;
  }

  // Use persisted prompt-token truth to choose a compactable boundary.
  // Exact boundary math is implementation-owned, but it must be anchored by
  // checkpointed messages rather than ad-hoc split heuristics alone.
  return selectPromptAnchoredBoundary(
    args.liveMessages,
    checkpoint,
    args.effectiveInputBudget,
  );
}

async function runCompletionLoop(state: {
  messages: Message[];
  compactSummary: CompactSummary;
  safeInputTokenLimit: number;
  configuredMaxOutputTokens?: number | null;
}) {
  while (true) {
    const preflightTokens = estimateNextPromptTokens({
      systemTokens: getSystemPromptTokens(),
      toolTokens: getToolSchemaTokens(),
      serviceContextTokens: getServiceContextTokens(),
      liveMessages: state.messages,
      compactSummary: state.compactSummary,
    });
    const measuredOutputTokensReserve = deriveMeasuredOutputTokensReserve({
      liveMessages: state.messages,
      configuredMaxOutputTokens: state.configuredMaxOutputTokens,
    });
    const totalBudgetTokens =
      preflightTokens + measuredOutputTokensReserve;
    const effectiveInputBudget = calculateEffectiveInputBudget({
      safeInputTokenLimit: state.safeInputTokenLimit,
      measuredOutputTokensReserve,
    });

    if (totalBudgetTokens >= state.safeInputTokenLimit) {
      const target = findCompactTarget({
        liveMessages: state.messages,
        effectiveInputBudget,
      });

      if (!target) {
        throw new InvalidContextStateError(
          'Prepared payload exceeds the effective context limit, but no prompt-token checkpoint can anchor compaction.',
        );
      }

      const compactResult = await runCompactionRequest({
        previousSummary: state.compactSummary,
        liveMessages: state.messages,
        compactToId: target.toId,
      });

      state.compactSummary = {
        toId: target.toId,
        summary: compactResult.summary,
        condensedCount: target.condensedCount,
      };

      state.messages = removeCompactedPrefix(state.messages, target.toId);
      continue;
    }

    const lastSubmittedInput = findLastSubmittedInputMessage(state.messages);

    const response = await submitNormalCompletion({
      compactSummary: state.compactSummary,
      liveMessages: state.messages,
    });

    if (lastSubmittedInput && response.usage?.promptTokens != null) {
      persistPromptTokensCheckpoint(
        lastSubmittedInput.id,
        response.usage.promptTokens,
      );
    }

    // Assistant usage metadata must be retained so the next preflight can derive
    // its measured output reserve from observed completionTokens.
    applyAssistantResult(state, response);
  }
}
```

---

## 10. Non-Negotiable Invariants

These rules are mandatory:

1. `message.promptTokens` must be persisted, not runtime-only.
2. The persisted value must come from provider `usage.promptTokens`.
3. The value must be written to the last submitted input message.
4. Assistant `usage.completionTokens` must be retained so output reserve can be
   derived from observed turns.
5. Preflight overflow must block the oversized normal request before send.
6. Preflight overflow is evaluated against total budget, not input-only estimate:
   `conservative_prompt_tokens + measured_output_tokens_reserve >= safe_input_token_limit`.
7. Compaction target selection must use the reserve-aware effective input budget,
   not the raw safe input limit.
8. Reserve derivation must prefer the maximum assistant `completionTokens` inside
   the latest external-request cycle, then fall back to the latest observed
   assistant completion usage, then configured output reserve capped at 8192.
9. Missing checkpoint anchors in an overflow situation must be treated as invalid
   state.
10. Frontend may assist UX, but backend owns correctness.
11. Summary bubble counts must describe the compacted delta actually submitted by
   the compaction payload, not unrelated totals.

---

## 11. Practical Interpretation

When reading logs:

- `conservative_prompt_tokens=...` means Rust's preflight estimate
- `measured_output_tokens_reserve=...` means the reserved output budget derived
  from observed assistant `completionTokens` or configured fallback
- `total_budget_tokens=...` means the actual preflight gate:
  `conservative_prompt_tokens + measured_output_tokens_reserve`
- `effective_input_budget=...` means the checkpoint-selection and compaction-fit
  budget after reserve subtraction
- `usage.promptTokens=...` means actual provider-submitted size for the request

When reading stored messages:

- `promptTokens=null` means the message is not a grounded checkpoint
- `promptTokens=number` means the message can serve as a persisted compaction anchor
- `usage.completionTokens=number` on assistant turns means the turn can
  participate in future output-reserve derivation

When debugging compaction:

1. check whether preflight blocked before send
2. check whether `total_budget_tokens` crossed `safe_input_token_limit`
3. check which assistant turn supplied the measured output reserve
4. check whether a prompt-token checkpoint existed
5. check which `to_id` was selected under the reserve-aware effective input budget
6. check that compact summary was stored
7. check that the resumed request was rebuilt with the summary injected
8. check that the summary bubble count matches the compacted delta actually kept
   after payload fitting

That is the current contract.
