# SP17 - Action Items for Production Readiness

## P0 - CRITICAL (Fix Before Deployment)

### 1. Fix Summary Message Role

**File**: `src/context/llm/useLLMExecution.ts:333`

**Current**:

```typescript
const summaryMessage: Message = {
  id: `compact-summary-${cached.fromId}~${cached.toId}`,
  sessionId,
  threadId: messages[0]?.threadId ?? sessionId,
  role: 'user', // ❌ Wrong - always 'user'
  content: [
    {
      type: 'text',
      text: `[Summary of previous conversation (from message ${cached.fromId} to ${cached.toId})]\n${cached.summary}`,
    },
  ],
};
```

**Fix**:

```typescript
const summaryMessage: Message = {
  id: `compact-summary-${cached.fromId}~${cached.toId}`,
  sessionId,
  threadId: messages[0]?.threadId ?? sessionId,
  role: 'system', // ✅ System context, not user input
  content: [
    {
      type: 'text',
      text: `[Context Summary from previous conversation (messages ${cached.fromId} to ${cached.toId})]\n\n${cached.summary}`,
    },
  ],
};
```

**Why**: LLM semantic correctness. A summary is system context, not a user message.

**Effort**: 5 minutes

---

### 2. Add Validation for fromId Before Using Cache

**File**: `src/context/llm/useLLMExecution.ts:326`

**Current**:

```typescript
const toIdIndex = messages.findIndex((m) => m.id === cached.toId);
if (toIdIndex >= 0) {
  const remainingMessages = messages.slice(toIdIndex + 1);
  // ... use cache
}
```

**Fix**:

```typescript
const toIdIndex = messages.findIndex((m) => m.id === cached.toId);
const fromIdIndex = messages.findIndex((m) => m.id === cached.fromId);

if (toIdIndex >= 0 && fromIdIndex >= 0 && fromIdIndex < toIdIndex) {
  const remainingMessages = messages.slice(toIdIndex + 1);
  // ... use cache
} else if (toIdIndex < 0 || fromIdIndex < 0) {
  logger.warn(
    'Stale compact cache: boundary IDs not found in history. Invalidating cache.',
    {
      sessionId,
      fromId: cached.fromId,
      toId: cached.toId,
      fromIdFound: fromIdIndex >= 0,
      toIdFound: toIdIndex >= 0,
    },
  );
  compactCacheRef.current.delete(sessionId);
}
```

**Why**: Prevents using corrupted cache if message history was pruned/reordered.

**Effort**: 10 minutes

---

### 3. Add Retry Logic for Compaction Failures

**File**: `src/context/llm/useLLMExecution.ts:466-523`

**Current**:

```typescript
(async () => {
  try {
    const summary = await service.compact(oldMessages, { modelName: model });
    // ... save and notify
  } catch (err) {
    logger.error('❌ Async compaction failed', { sessionId, error: err });
  } finally {
    const resolvers = compactResolversRef.current.get(sessionId) ?? [];
    resolvers.forEach((r) => r());
    compactResolversRef.current.delete(sessionId);
  }
})();
```

**Fix**:

```typescript
const attemptCompaction = async (attempt = 1) => {
  try {
    const summary = await service.compact(oldMessages, { modelName: model });
    // ... save and notify normally
  } catch (err) {
    const maxAttempts = 3;
    const backoffMs = Math.min(5000 * Math.pow(2, attempt - 1), 30000);

    if (attempt < maxAttempts) {
      logger.warn('Compaction failed, retrying...', {
        sessionId,
        attempt,
        nextRetryMs: backoffMs,
        error: err,
      });
      setTimeout(() => attemptCompaction(attempt + 1), backoffMs);
      return;
    }

    logger.error('❌ Async compaction failed after max retries', {
      sessionId,
      attempts: maxAttempts,
      error: err,
    });

    // Notify waiters that compaction failed - they should trim messages manually
    const resolvers = compactResolversRef.current.get(sessionId) ?? [];
    resolvers.forEach((r) => r());
    compactResolversRef.current.delete(sessionId);
  }
};

(async () => {
  await attemptCompaction();
})();
```

**Why**: Prevents silent context loss when compaction API temporarily fails.

**Effort**: 15 minutes

---

## P1 - IMPORTANT (Fix Soon)

### 4. Log Model Metadata Fallback

**File**: `src/context/llm/useLLMExecution.ts:243-256`

**Current**:

```typescript
let modelInfo: ModelInfo | null =
  (await service.listModels()).find((m) => m.name === model) || null;
if (!modelInfo) {
  modelInfo =
    llmConfigManager.getModel(provider, model) ??
    ({
      contextWindow: 64 * 1024,
      supportReasoning: false,
      supportTools: false,
      cost: { input: 0, output: 0 },
      name: model,
    } as ModelInfo);
}
```

**Fix**:

```typescript
let modelInfo: ModelInfo | null =
  (await service.listModels()).find((m) => m.name === model) || null;

if (!modelInfo) {
  modelInfo = llmConfigManager.getModel(provider, model);

  if (!modelInfo) {
    logger.warn(
      '⚠️ Model metadata not found - using conservative 64KB default context window. ' +
        'This may cause unnecessary compaction.',
      { provider, model },
    );

    modelInfo = {
      contextWindow: 64 * 1024,
      supportReasoning: false,
      supportTools: false,
      cost: { input: 0, output: 0 },
      name: model,
    } as ModelInfo;
  }
}
```

**Why**: Users need to know when using conservative defaults.

**Effort**: 5 minutes

---

### 5. Add Minimum Compaction Savings Threshold

**File**: `src/context/llm/useLLMExecution.ts:419-462`

**Current**:

```typescript
if (oldMessages.length >= 5 || (cached && oldMessages.length > 1)) {
  compactResolversRef.current.set(sessionId, []);
  // Trigger compaction
}
```

**Fix**:

```typescript
// Only compact if it saves meaningful tokens (typical summary is ~500 tokens, so only worth if compacting 1000+ tokens)
const compactSavings = totalTokens - currentSum; // tokens in messages to compact
const minCompactThreshold = 1500; // Only if would save >1500 tokens

if (
  (oldMessages.length >= 5 || (cached && oldMessages.length > 1)) &&
  compactSavings >= minCompactThreshold
) {
  logger.info('Compaction savings justified', {
    sessionId,
    savingsTokens: compactSavings,
    minThreshold: minCompactThreshold,
  });
  compactResolversRef.current.set(sessionId, []);
  // Trigger compaction
} else if (oldMessages.length >= 5 || (cached && oldMessages.length > 1)) {
  logger.debug('Compaction not triggered - insufficient savings', {
    sessionId,
    savingsTokens: compactSavings,
    minThreshold: minCompactThreshold,
  });
}
```

**Why**: Prevents expensive summarization calls that don't actually reduce tokens.

**Effort**: 10 minutes

---

### 6. Consolidate Token Estimation (Lines 364-368 + 600-609)

**File**: `src/context/llm/useLLMExecution.ts`

**Current**: Token counts calculated twice for compact mode.

**Fix**: Cache the result from first calculation:

```typescript
// After line 368, save the result:
const cachedTotalTokens = totalTokens;
const cachedThreshold = threshold;

// Later at line 605, reuse instead of recalculating:
// const totalEstimatedTokens = calculateGroundedTotalTokens(...)
// Replace with: (only for window mode, not compact mode)
```

**Why**: Single source of truth, avoid rounding errors.

**Effort**: 10 minutes

---

### 7. Potential Race Condition - Add Comment/Documentation

**File**: `src/context/llm/useLLMExecution.ts:403-407, 513-516`

**Current**: No explanation of the resolver queue pattern.

**Fix**: Add JSDoc comment:

```typescript
/**
 * Promise-based waiting mechanism for overflow prevention.
 *
 * Pattern:
 * 1. When context overflows (>100%) and compaction is pending, new requests
 *    queue a resolver callback in compactResolversRef
 * 2. Async compaction runs in background via IIFE
 * 3. When compaction finishes (success/failure), all queued resolvers fire
 *    and waiting requests wake up with fresh context
 *
 * Safety notes:
 * - All resolvers always fire, even on compaction error (finally block)
 * - Multiple requests can queue simultaneously (Map<sessionId, resolve[]>)
 * - Waiting requests don't consume tokens during await (no double-counting)
 */
```

**Why**: Prevent future developers misunderstanding the pattern.

**Effort**: 5 minutes

---

## P2 - NICE-TO-HAVE (Future)

### 8. Track Compaction Cost

Add token usage return from `compact()` method and log savings ratio.

### 9. Recursive Summary Support

For sessions lasting weeks, support summary-of-summary chains.

### 10. Per-Model Compaction Prompts

Tailor summarization style to each model's strengths.

---

## Testing Checklist

After fixes, add tests for:

- [ ] Stale cache detection (fromId + toId)
- [ ] Compaction retry with exponential backoff
- [ ] Overflow waiting with 3+ concurrent requests
- [ ] Model metadata fallback chain
- [ ] E2E persistence across app restart
- [ ] Summary message role is 'system', not 'user'
- [ ] Minimum compaction threshold enforcement

---

## Summary

| Priority | Count | Est. Time | Status              |
| -------- | ----- | --------- | ------------------- |
| P0       | 3     | 30 min    | **Must complete**   |
| P1       | 4     | 40 min    | **Should complete** |
| P2       | 3     | TBD       | **Future**          |

**Total Estimated Time**: 70 minutes (1.2 hours) to production-ready
