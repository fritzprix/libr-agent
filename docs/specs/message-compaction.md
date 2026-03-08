# Spec: Message Context Compaction (SP17)

## 1. Overview

An intelligent message management system that resolves context window saturation as conversations grow. It summarises old turns and preserves recent context, enabling arbitrarily long sessions without hitting model limits.

**Context strategies** (configured via `contextStrategy` in Settings):

- **`window`** (default): sliding window — passes the `windowSize` most recent messages to the LLM. Simple, no summarisation.
- **`compact`**: async compaction — detects token threshold, summarises old messages in the background, always passes a [summary + recent messages] stack. This spec describes `compact` exclusively.

---

## 2. Core Logic

### 2.1 Effective Limit Calculation (`calculateEffectiveContextLimit`)

The system derives a single `effectiveLimit` (= `safeInputTokenLimit`) that drives all downstream logic:

```
safeInputLimit  = modelMax - (defaultMaxOutputTokens + 100)   // reserve output budget + safety buffer
effectiveLimit  = min(safeInputLimit, maxInputContext)          // respect user-configured cap
```

- **`safeInputTokenLimit`** is used directly as the overflow boundary (100%) and as the basis for the 90% compaction threshold. There is **no additional safety factor** applied on top.
- **Default fallback** when model context window is unknown: 64 × 1024 = 65 536 tokens.

### 2.2 Token Estimation (`calculateGroundedTotalTokens`)

To minimise BPE drift over long sessions, the system uses the last API-reported `usage.totalTokens` as a ground truth anchor and only estimates _incremental_ new messages via BPE.

| Case                                                                                                | Behaviour                                                                   |
| --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| **Grounded** (most recent `assistant` message has `usage.totalTokens > 0`)                          | `base = usage.totalTokens` + BPE for all messages after that assistant turn |
| **Post-compaction fallback** (a `compact-summary-*` message exists _after_ the last grounded point) | Full BPE re-estimation across all messages in the candidate stack           |
| **Cold start fallback** (no grounded message in history)                                            | Full BPE re-estimation                                                      |

**`estimateTokensBPE` counts all token-bearing fields on a `Message`:**

- `content[]` items: `type:'text'` → `.text`; `type:'resource'` → `.resource.text`; `type:'tool_call'` → `.name + .arguments`; `type:'thinking'` → `.thinking`
- `message.tool_calls[]` (OpenAI-style top-level field): `function.name + function.arguments`
- `message.tool_use` (Anthropic-style top-level field): `name + JSON.stringify(input)`
- `message.thinking` (top-level thinking string)
- Binary content types (`image`, `audio`, `video`) are not counted.

### 2.3 Compaction Trigger & Split (`findCompactionSplitIndex`)

**Trigger condition** (evaluated on every LLM call, compact strategy only):

- `totalTokens >= threshold` (90% of `effectiveLimit`) **AND**
- no compaction is already in progress for this session (`!compactResolversRef.current.has(sessionId)`)

**Split index calculation:**

1. **Budget-based**: Compute `messageBudget = threshold − systemPromptTokens − toolsTokens`. Walk messages newest-to-oldest, accumulating token estimates until the sum reaches `keepThreshold = max(1000, messageBudget × 0.5)`. The first message that crosses the threshold becomes the split point.
2. **Forced fallback**: If the accumulation loop ends with `splitIdx === 0` AND `messages.length >= 10`, force `splitIdx = floor(messages.length / 2)`.

**Execution condition** — compaction only fires if:

- `oldMessages.length >= 5` (fresh compaction), OR
- An existing `compactCache` entry exists for the session AND `oldMessages.length > 1` (incremental extension of a prior summary).

### 2.4 Orchestration & Synchronisation (`useLLMExecution.ts`)

Executed on every LLM send in `compact` mode, in order:

#### Step 1 — Session Resume / Cache Hydration

If `compactCacheRef` does not have an entry for the session (e.g., first send after page load), the system queries the backend DB via `compactContextService.getCompactContext(sessionId)`. If a persisted record exists, it is hydrated into `compactCacheRef` and `compactedRangeMap`.

#### Step 2 — Build Candidate Stack (`buildCandidateStack`)

Constructs the message list that will be sent to the LLM:

- If no cached summary → full raw message list.
- If cache exists and `toId` is found in `messages` → inject a synthetic **summary message** immediately before `messages[toIdIndex + 1]`, then drop all messages up to and including `toId`.
- If cache exists but `toId` is **not** found (stale — message deleted) → invalidate cache and use full message list.

The synthetic summary message has:

- `role: 'user'` (ensures universal provider compatibility)
- `id: compact-summary-{displayFromId}~{toId}`
- `content[0].text`: `[Summary of previous conversation (from message {displayFromId} to {toId})]\n{summary}`

#### Step 3 — Token Count & Overflow Check

Compute `totalTokens` via `calculateGroundedTotalTokens`. Then:

- **Reserved-tokens overflow guard**: if `systemPromptTokens + toolsTokens >= safeInputTokenLimit`, emit a `toast.warning` (Sonner, stable ID per session) and log a warning. Compaction cannot help — the fixed cost alone exceeds the limit. Execution continues; `selectMessagesWithinContext` will hard-trim to the most-recent message.
- `overflow = totalTokens >= safeInputTokenLimit` (100%)

#### Step 4 — Await Pending Compaction (if overflow)

If `overflow && compactResolversRef.current.has(sessionId)` (in-progress compaction):

1. Add session to `awaitingSet` (triggers UI "waiting" indicator).
2. Block on a `Promise<boolean>` whose resolver is queued in `compactResolversRef`.
3. On resolution:
   - `true` (success) → rebuild `candidateMessages` from fresh cache, recalculate `totalTokens`/`overflow`.
   - `false` (failure) → log warning; proceed with the oversized stack; `selectMessagesWithinContext` will hard-trim.
4. Remove session from `awaitingSet`.

#### Step 5 — Update Context Usage Gauge

Update `contextUsageMap` with `{ totalTokens, contextWindow: safeInputTokenLimit, modelMaxContext: modelMaxLimit }` for UI gauge rendering.

#### Step 6 — Trigger Async Compaction (if threshold)

If trigger condition is met (§2.3): set `compactResolversRef` to empty array, add session to `compactingSet`, then launch fire-and-forget async IIFE:

1. Call `service.compact(oldMessages, { modelName: model })` — uses the **same model** the session is currently configured for.
2. On success: write new `{ fromId, toId, summary }` to `compactCacheRef`, persist via `compactContextService.saveCompactContext`, update `compactedRangeMap`.
3. On failure: log error.
4. `finally`: resolve all queued promises with `compactionSucceeded`, delete session from `compactResolversRef` and `compactingSet`.

#### Step 7 — Final Message Selection

`selectMessagesWithinContext(candidateMessages, ...)` hard-trims the candidate stack (which may still be oversized if compaction is still running) to fit within `safeInputTokenLimit`, accounting for `systemPrompt` and `toolsJson`. Messages are also passed through `MessageNormalizer.sanitizeMessagesForProvider`.

### 2.5 Summarisation (`compact()` in `BaseAIService`)

`buildCompactPrompt(messages)` serialises the slice to summarise as plain text:

- `user` → `User: {text}`
- `assistant` with `tool_calls` → `Assistant (called tools: {names}): {text}`
- `assistant` without tool_calls → `Assistant: {text}`
- `tool` → `Tool result: {text}`

The prompt asks the model to _"Summarise concisely, preserving key decisions, context, tool results, and any information needed to continue the conversation."_ The first `type:'text'` block in the response is used as the summary string.

---

## 3. Data Integrity (Idempotency & Safety)

### 3.1 Deterministic Summary ID

- **Format**: `compact-summary-{OriginalFromId}~{toId}`
- **Nesting prevention**: Before computing `fromId` (in both `buildCandidateStack` and the async IIFE), the system strips any existing `compact-summary-` prefix from `firstMsg.id`, extracting the original message ID. This ensures re-compaction of a summary-headed stack still produces a flat, non-nested ID.

### 3.2 Stale Cache Protection

If `cached.toId` is not found in the current `messages` array (e.g., message was deleted), the cache entry is invalidated immediately and the full uncompacted history is used as the candidate stack.

### 3.3 Tool Budget Accounting

`selectMessagesWithinContext` always receives `toolsJson` so that tool definition tokens are correctly reserved before message-budget calculations.

### 3.4 Session Delete Cleanup (`clearSessionState`)

When a session is deleted (`deleteSession` BFS or `deleteSessionOnly`), `clearSessionState(sessionId)` is called, which removes the session's entries from both `compactCacheRef` and `compactResolversRef`. This prevents stale in-memory state from accumulating and avoids resolver leaks for any in-flight compaction.

---

## 4. UI Representation

| Component                                                  | Behaviour                                                                                                                                                |
| ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Context gauge** (`ContextGauge` in `AgentChatStatusBar`) | Denominator = `effectiveLimit`; red zone = ≥ 90% (matches compaction trigger point). Tooltip shows both `Effective Limit` and `Model Max`.               |
| **Compacting indicator**                                   | `isCompacting(sessionId)` drives a visual badge in `AgentChatStatusBar` while async compaction is in progress.                                           |
| **Awaiting indicator**                                     | `isAwaitingCompact(sessionId)` drives a visual badge while an LLM send is blocked waiting for compaction to complete.                                    |
| **`CompactEventDivider`**                                  | Rendered in `AgentChatMessages` **after** the message whose `id === compactedRange.toId`. Displays "Context compressed above" with a `DatabaseZap` icon. |
| **Context overflow toast**                                 | `toast.warning('Context window too small', ...)` via Sonner, with a stable `id` per session to prevent stacking on repeated sends.                       |
