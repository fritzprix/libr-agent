# Message Compaction Contract

## Quick Read

This document defines the current compaction contract for Agent V2.

The short version:

1. Provider `usage.promptTokens` is the only grounded full-prompt truth.
2. That value belongs to the **last submitted real input message** of a successful
   request.
3. Next-turn fit starts from the last grounded `usage.promptTokens` and only adds
   known post-checkpoint growth.
4. If the request does not fit, Rust compacts before send using a checkpoint-anchored
   causal-prefix strategy.
5. Successful compaction must invalidate stale retained-tail checkpoints and set
   runtime `lastReportedPromptTokens = null`.
6. Empty compaction responses still enter the bounded soft-retry ladder; only after
   that ladder is exhausted may Rust persist a deterministic hard fallback summary.
7. If the request does not fit and there is no usable checkpoint candidate, the
   state is invalid and the backend must reject without mutating history.

The old mental model around `safeCheckpointId` is obsolete. Context-fit truth must
survive restart and later `maxInputContext` changes, so the checkpoint must live on
messages themselves.

---

## 1. Scope

This spec covers:

- persisted token checkpoints
- request-time preflight behavior
- compaction trigger behavior
- bounded retry and hard-fallback behavior
- invalid-state handling
- compact summary persistence and reinjection
- fallback artifact spillover and resume behavior
- UI-facing condensed-count meaning

This spec does **not** define provider-specific SDK request formatting.

---

## 2. Core Contract

### 2.1 Source of truth

There are two different signals and they must not be confused:

1. **Provider-reported prompt truth**
   - `usage.promptTokens` is the actual full submitted input size for a completed
     request.
   - This is the only grounded prompt-size truth.
2. **Rust preflight projection**
   - Rust projects the next request by starting from the last grounded
     `usage.promptTokens` and adding only known post-checkpoint growth.
   - Dynamic service-context growth can still make the next real request larger
     than this projection.

Important consequence:

```text
next-turn projection != guaranteed next submitted size
```

If Rust preflight blocks a request, that blocked request was never submitted to the
provider.

### 2.2 Persistent checkpoint rule

After a successful completion request, the backend must persist the provider's
`usage.promptTokens` onto the **last submitted real input message** for that request.

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

Rust projects the next request from:

1. the last grounded `usage.promptTokens`
2. known post-checkpoint growth
3. the current runtime `maxInputContext`

If the projected next request is within the safe limit:

```text
send normal completion request
```

If the projected next request exceeds the safe limit:

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
start from the newest usable checkpoint candidate below the current request tail,
compact the older live causal prefix into summary state, and back off toward older
candidates if dynamic prompt growth makes the first split still fail
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

### 5.4 Post-compaction invalidation

After a compaction step succeeds:

1. any `promptTokens` still present on the retained tail are stale full-prompt
   measurements from the pre-compaction epoch and must be invalidated for checkpoint
   reuse
2. runtime `lastReportedPromptTokens` must be set to `null`
3. only a later successful submit may establish new grounded prompt truth

### 5.5 Bounded retry policy

Dynamic service-context growth can invalidate the most recent checkpoint-based split.

Therefore compaction must:

1. try the newest usable checkpoint candidate first
2. back off one candidate at a time toward older prefix boundaries
3. treat an empty compaction response as a recoverable compaction failure while the
   recovery ladder still has room
4. stop after the bounded soft-retry ladder is exhausted:
   `CacheAligned -> OverflowRecovery -> DegradedTools`
5. if the ladder is exhausted but a compaction boundary did exist, persist a
   deterministic hard fallback summary with fixed handoff sections instead of
   deadlocking the workflow
6. if the artifact spill write fails, keep the fallback summary path alive anyway;
   artifact persistence is best-effort, not a gate on resume

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
4. If the UI keeps showing token metrics during `awaitingCompact` or `compacting`,
   it should prefer the last stable preflight projection over a transient blocked
   overflow value so the badge does not imply the compaction request itself
   overflowed.

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

Quick mental model first:

```ts
function runTurn(state) {
  const projectedPromptLoad = estimateNextPromptLoadFromLastTruth(state);

  if (
    projectedPromptLoad == null ||
    projectedPromptLoad >= state.maxInputContext
  ) {
    const splitCandidates = newestCheckpointAnchorsFirst(state.liveMessages);

    if (splitCandidates.length === 0) {
      throw InvalidContextStateError();
    }

    const result = compactWithBoundedRecovery(state, splitCandidates.slice(0, 3));

    if (result.kind === 'success') {
      persistCompactedSummary(result.summary, result.toId);
      dropCompactedPrefixFromLiveHistory(state, result.toId);
      invalidateRetainedTailPromptTruth(state);
      return runTurn(state); // retry with compact summary injected
    }

    if (result.kind === 'hard_fallback') {
      persistDeterministicFallbackSummary(result.summary, result.toId);
      dropCompactedPrefixFromLiveHistory(state, result.toId);
      invalidateRetainedTailPromptTruth(state);
      return runTurn(state); // retry with fallback summary injected
    }

    throw InvalidContextStateError();
  }

  const response = submitNormalCompletion(state);
  persistProviderPromptTruthOnLastSubmittedRealInput(response);
  appendAssistantResult(state, response);
}
```

This is the simplified control-flow view. The full intended behavior model is below.

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

const MAX_SPLIT_TRIES = 3;
const RECOVERY_PHASES = [
  'CacheAligned',
  'OverflowRecovery',
  'DegradedTools',
] as const;

function selectLastSubmittedRealInputMessage(
  messages: Message[],
): Message | null {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    if (messages[i].source !== 'internal') {
      return messages[i];
    }
  }
  return null;
}

function projectedNextPromptLoad(args: {
  lastReportedPromptTokens: number | null;
  outputReserveTokens: number;
  toolResultGrowthTokens: number;
  serviceContextGrowthTokens: number;
}): number | null {
  if (args.lastReportedPromptTokens == null) {
    return null;
  }

  return (
    args.lastReportedPromptTokens +
    args.outputReserveTokens +
    args.toolResultGrowthTokens +
    args.serviceContextGrowthTokens
  );
}

function buildCheckpointBackoffCandidates(
  liveMessages: Message[],
): { toId: string; condensedCount: number }[] {
  return selectCheckpointAnchoredBoundariesNewestFirst(liveMessages).filter(
    (candidate) =>
      retainedTailPreservesToolOwnership(liveMessages, candidate.toId),
  );
}

function invalidateRetainedTailPromptTokens(messages: Message[]): Message[] {
  return messages.map((message) => ({
    ...message,
    promptTokens: null,
  }));
}

async function runCompletionLoop(state: {
  messages: Message[];
  compactSummary: CompactSummary;
  maxInputContext: number;
  lastReportedPromptTokens: number | null;
}) {
  while (true) {
    const projectedPromptLoad = projectedNextPromptLoad({
      lastReportedPromptTokens: state.lastReportedPromptTokens,
      outputReserveTokens: deriveOutputReserve(state.messages),
      toolResultGrowthTokens: deriveToolResultGrowth(state.messages),
      serviceContextGrowthTokens: deriveServiceContextGrowth(),
    });

    const mustCompact =
      projectedPromptLoad == null ||
      projectedPromptLoad >= state.maxInputContext;

    if (mustCompact) {
      const candidates = buildCheckpointBackoffCandidates(state.messages);

      if (candidates.length === 0) {
        throw new InvalidContextStateError(
          'Prepared payload exceeds the prompt limit, but no prompt-token checkpoint can anchor compaction.',
        );
      }

      const compactResult = await runBoundedCompactionRecovery({
        previousSummary: state.compactSummary,
        liveMessages: state.messages,
        candidates: candidates.slice(0, MAX_SPLIT_TRIES),
        recoveryPhases: RECOVERY_PHASES,
      });

      if (compactResult.kind === 'success') {
        state.compactSummary = {
          toId: compactResult.toId,
          summary: compactResult.summary,
          condensedCount: compactResult.condensedCount,
        };
        state.messages = invalidateRetainedTailPromptTokens(
          removeCompactedPrefix(state.messages, compactResult.toId),
        );
        state.lastReportedPromptTokens = null;
        continue;
      }

      if (compactResult.kind === 'hard_fallback') {
        state.compactSummary = {
          toId: compactResult.toId,
          summary: compactResult.summary,
          condensedCount: compactResult.condensedCount,
        };
        state.messages = invalidateRetainedTailPromptTokens(
          removeCompactedPrefix(state.messages, compactResult.toId),
        );
        state.lastReportedPromptTokens = null;
        continue;
      }

      throw new InvalidContextStateError(
        'Prepared payload still exceeds the prompt limit, and no usable compaction boundary exists.',
      );
    }

    const lastSubmittedInput = selectLastSubmittedRealInputMessage(
      state.messages,
    );

    const response = await submitNormalCompletion({
      compactSummary: state.compactSummary,
      liveMessages: state.messages,
    });

    if (lastSubmittedInput && response.usage?.promptTokens != null) {
      persistPromptTokensCheckpoint(
        lastSubmittedInput.id,
        response.usage.promptTokens,
      );
      state.lastReportedPromptTokens = response.usage.promptTokens;
    }

    applyAssistantResult(state, response);
  }
}
```

---

## 10. Compaction Instruction Seed Contract

Compaction request assembly has two separate inputs:

1. **Compaction body** — the prefix actually being summarized (`split_idx` / replayed
   request layout / overflow recovery subject)
2. **Instruction seed** — the latest external request and nearby reference context
   taken from the **full pre-compaction message stack**, even when that request is
   outside the compacted body window

Pseudocode:

```ts
function buildCompactionInstructionInput(args: {
  allMessages: Message[];
  compactBodyMessages: Message[];
}): {
  hasPriorSummary: boolean;
  priorSummary: Message | null;
  latestExternalRequestMessages: Message[];
  referenceContextMessages: Message[];
} {
  const latestExternalRequestRange = findLatestExternalRequestSeedBlockRange(
    args.allMessages,
  );

  if (latestExternalRequestRange == null) {
    return {
      hasPriorSummary:
        args.compactBodyMessages[0]?.source === 'compact_summary',
      priorSummary:
        args.compactBodyMessages.find(
          (message) => message.source === 'compact_summary',
        ) ?? null,
      latestExternalRequestMessages: [],
      referenceContextMessages: [],
    };
  }

  const [start, end] = latestExternalRequestRange;
  return {
    hasPriorSummary: args.compactBodyMessages[0]?.source === 'compact_summary',
    priorSummary:
      args.compactBodyMessages.find(
        (message) => message.source === 'compact_summary',
      ) ?? null,
    latestExternalRequestMessages: args.allMessages.slice(start, end),
    referenceContextMessages: args.allMessages.slice(
      Math.max(0, start - REFERENCE_CONTEXT_WINDOW_MESSAGES),
      end,
    ),
  };
}
```

This split is mandatory: the latest external request must remain explicitly visible
to the compaction summary instruction even when FIFO body trimming or checkpoint
splits would otherwise exclude it from the compacted prefix itself.

---

## 11. Non-Negotiable Invariants

These rules are mandatory:

1. `message.promptTokens` must be persisted, not runtime-only.
2. The persisted value must come from provider `usage.promptTokens`.
3. The value must be written to the last submitted real input message.
4. Assistant `usage.completionTokens` must be retained so output reserve can be
   derived from observed turns.
5. Preflight overflow must block the oversized normal request before send.
6. Next-turn fit decisions must start from grounded provider `usage.promptTokens`
   and add only known post-checkpoint growth; they must not treat checkpoints as
   per-message additive deltas.
7. Compaction target selection must start from the newest usable checkpoint and
   support bounded backoff toward older candidates.
8. A compaction boundary is invalid if the retained tail would break
   assistant/tool ownership.
9. After compaction, retained-tail `promptTokens` must be invalidated for checkpoint
   reuse.
10. After compaction, runtime `lastReportedPromptTokens` must be set to `null`.
11. Dynamic service-context growth must be tolerated via bounded retry rather than
    assuming the newest checkpoint split always fits.
12. Compaction instruction seeds must be derived from the full pre-compaction
    message stack, not only from the compacted body window.
13. Live latest-external-request bullets must be added before prior-summary
    carry-forward bullets so the current request cannot be evicted by a saturated
    prior summary `Active Request` section.
14. Missing checkpoint anchors in an overflow situation must be treated as invalid
    state.
15. Frontend may assist UX, but backend owns correctness.
16. Summary bubble counts must describe the compacted delta actually submitted by
    the compaction payload, not unrelated totals.
17. Empty compaction responses must flow through the bounded soft-retry ladder
    before hard fallback is allowed.
18. If bounded recovery is exhausted but a valid compaction boundary existed,
    compaction must persist a deterministic fallback summary, resume from it,
    and keep the fallback handoff shape stable.
19. Hard-fallback artifact spillover is best-effort; artifact-write failure must
    not cancel fallback summary persistence.
20. Stale compaction responses whose `to_id` no longer matches the active request
    must be ignored.

---

## 12. Practical Interpretation

When reading logs:

- `usage.promptTokens=...` means actual provider-submitted full prompt size for the
  request
- `conservative_prompt_tokens=...` is a Rust projection signal, not grounded truth
- dynamic service-context growth can make the next real prompt larger than a naive
  checkpoint projection
- `empty response from streamChat` during compaction is a retry signal while the
  recovery ladder still has phases left; it is not an immediate terminal failure
- `lastReportedPromptTokens=null` after compaction means the previous full-prompt
  truth was intentionally invalidated for the new epoch
- `stale compaction response ignored` means a late response lost the `to_id` race
  and was intentionally discarded

When reading stored messages:

- `promptTokens=null` means the message is not currently a grounded checkpoint
- `promptTokens=number` means the message may serve as a persisted compaction anchor
  if it is still usable in the current live range
- `usage.completionTokens=number` on assistant turns means the turn can
  participate in future output-reserve derivation
- the latest external request may be preserved in the compaction instruction even
  when it is not part of the compacted body prefix
- a fallback compact summary may include artifact path/guidance when spillover
  succeeded; if not, the summary itself is the only saved handoff

When debugging compaction:

1. check whether preflight blocked before send
2. check what the last grounded `usage.promptTokens` value was
3. check whether dynamic service-context growth or tool-result growth invalidated a
   naive checkpoint fit assumption
4. check whether a usable prompt-token checkpoint existed in the current live range
5. check which `to_id` candidate was selected and whether backoff to older
   candidates was attempted
6. check whether the retained tail preserved assistant/tool ownership
7. check whether the instruction seed came from the full live message stack or
   was accidentally derived only from the compacted prefix
8. check that the latest external request appears in the instruction seed even if
   it sits outside the compacted body window
9. check that compact summary was stored
10. check that retained-tail `promptTokens` were invalidated and
    `lastReportedPromptTokens` was cleared
11. check that the resumed request was rebuilt with the summary injected
12. check that the summary bubble count matches the compacted delta actually kept
    after payload fitting
13. if the UI badge stayed visible during compaction, check whether it intentionally
    held the last stable preflight value rather than the blocked overflow estimate
14. if compaction still could not stabilize, check whether hard fallback persisted
    a deterministic summary with optional artifact guidance instead of deadlocking
15. if the fallback summary lacks an artifact path, check whether the best-effort
    spill write failed while summary persistence still succeeded

That is the current contract.
