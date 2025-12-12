# Message Deduplication - Verification Guide

## Quick Verification Steps

### 1. Check Files Created

```bash
# Core implementation
ls src/lib/message-deduplicator.ts

# Test suite
ls src/lib/message-deduplicator.test.ts

# Documentation
ls IMPLEMENTATION_SUMMARY_DEDUPLICATION.md
ls DEDUPLICATION_VISUAL_GUIDE.md
```

### 2. Run Tests

```bash
# Run the deduplication test suite
pnpm test src/lib/message-deduplicator.test.ts

# Expected output:
# ✓ should return messages unchanged if count is below minimum
# ✓ should deduplicate repeated error messages
# ✓ should deduplicate repeated successful reads
# ✓ should preserve recent N messages
# ✓ should not break tool_call_id pairing
#
# Test Files  1 passed (1)
# Tests  5 passed (5)
```

### 3. Verify Integration

```bash
# Check import in use-ai-service.ts
grep -n "deduplicateToolCallPairs" src/hooks/use-ai-service.ts

# Expected output:
# 15:import { deduplicateToolCallPairs } from '@/lib/message-deduplicator';
# 190:        const deduplicatedMessages = deduplicateToolCallPairs(validMessages, {
```

### 4. Code Quality Check

```bash
# Run linter
pnpm lint

# Expected: No errors related to message-deduplicator.ts

# Check formatting
pnpm format:check

# Expected: Files are properly formatted
```

## Runtime Verification

### Enable Debug Logging

To see deduplication in action, the logger is already configured to output debug messages:

```typescript
// In src/lib/message-deduplicator.ts (line 155-157)
logger.debug(
  `Deduplicated ${totalRemoved} messages from ${uniqueHashes} unique tool call patterns`,
);
```

### Test in Development

1. **Start the app:**

   ```bash
   pnpm tauri dev
   ```

2. **Create a test scenario:**
   - Use an AI agent that calls tools
   - Make it try to read a non-existent file multiple times
   - Or use a tool that returns consistent results

3. **Check the console:**
   Look for log messages like:
   ```
   [MessageDeduplicator] Deduplicated 4 messages from 2 unique tool call patterns
   ```

### Example Test Scenario

Create a chat session and send:

```
User: "Try to read the file 'nonexistent.txt' three times and tell me what happens"
```

The AI will likely:

1. Call `read_file("nonexistent.txt")` → Error
2. Call `read_file("nonexistent.txt")` → Error (DEDUPLICATED)
3. Call `read_file("nonexistent.txt")` → Error (DEDUPLICATED)

**Expected behavior:**

- Only the first error pair is sent to the LLM on subsequent requests
- The tool message shows "(repeated 3x)"
- Metadata includes `dedupCount: 3`
- Debug log shows: "Deduplicated 4 messages from 1 unique tool call patterns"

## Performance Verification

### Measure Impact

Add timing logs to `src/hooks/use-ai-service.ts` (temporary):

```typescript
// Before deduplication (line 189)
const startTime = performance.now();

const deduplicatedMessages = deduplicateToolCallPairs(validMessages, {
  preserveRecentN: 3,
  minMessageCount: 10,
});

const dedupTime = performance.now() - startTime;
logger.debug(`Deduplication took ${dedupTime.toFixed(2)}ms`);
```

**Expected results:**

- < 1ms for small chats (< 10 messages)
- < 5ms for typical chats (50 messages)
- < 20ms for large chats (200 messages)

### Token Savings Measurement

To see actual token savings, add logging before and after:

```typescript
// In use-ai-service.ts, around line 189
logger.info(`Messages before dedup: ${validMessages.length}`);

const deduplicatedMessages = deduplicateToolCallPairs(validMessages, {
  preserveRecentN: 3,
  minMessageCount: 10,
});

logger.info(`Messages after dedup: ${deduplicatedMessages.length}`);
logger.info(
  `Removed ${validMessages.length - deduplicatedMessages.length} messages`,
);
```

## Manual Inspection

### Check Message Structure

Add a temporary log to see the deduplicated message:

```typescript
// In message-deduplicator.ts, line 122
if (matchingPair && matchingPair.count > 1) {
  const updatedMessage: Message = {
    ...message,
    metadata: {
      ...message.metadata,
      dedupCount: matchingPair.count,
    },
  };

  // Temporary debug log
  console.log('Deduplicated message:', JSON.stringify(updatedMessage, null, 2));

  // ... rest of the code
}
```

**Expected output:**

```json
{
  "id": "msg_123",
  "role": "tool",
  "tool_call_id": "call_456",
  "content": [
    {
      "type": "text",
      "text": "Error: File not found (repeated 3x)"
    }
  ],
  "metadata": {
    "dedupCount": 3
  }
}
```

## Edge Case Verification

### Test 1: Small Message Count (Early Exit)

```typescript
// Should skip deduplication
const messages = [
  { id: '1', role: 'user', ... },
  { id: '2', role: 'assistant', tool_calls: [...], ... },
  { id: '3', role: 'tool', ... },
];

const result = deduplicateToolCallPairs(messages, {
  minMessageCount: 10,
  preserveRecentN: 3,
});

// Expected: result.length === messages.length (no changes)
```

### Test 2: Recent Message Preservation

```typescript
// Messages 8, 9, 10 should be preserved even if duplicates
const messages = [
  /* 10 messages, last 3 are duplicates */
];

const result = deduplicateToolCallPairs(messages, {
  minMessageCount: 10,
  preserveRecentN: 3,
});

// Expected: Last 3 messages unchanged
```

### Test 3: Different Results (No Dedup)

```typescript
// Same tool call, different responses - should NOT deduplicate
const messages = [
  assistantMessage('call_1', 'read_file', '{"path":"log.txt"}'),
  toolMessage('call_1', 'Line 1\nLine 2'),
  assistantMessage('call_2', 'read_file', '{"path":"log.txt"}'),
  toolMessage('call_2', 'Line 1\nLine 2\nLine 3'), // Changed!
  // ... padding to reach 10 messages
];

const result = deduplicateToolCallPairs(messages);

// Expected: No deduplication (different content)
```

## Validation Checklist

- [ ] All 5 tests pass
- [ ] No TypeScript compilation errors
- [ ] No ESLint errors
- [ ] Properly formatted code
- [ ] Integration in use-ai-service.ts confirmed
- [ ] Debug logging works
- [ ] Early exit works (< 10 messages)
- [ ] Recent messages preserved
- [ ] Metadata added correctly
- [ ] Content annotation works ("repeated Nx")
- [ ] Performance acceptable (< 20ms for 200 messages)
- [ ] Works with error messages
- [ ] Works with successful responses
- [ ] No orphaned tool messages
- [ ] tool_call_id pairing preserved

## Troubleshooting

### Issue: Tests fail with "Cannot find module"

**Solution:** Ensure vitest is installed and configured:

```bash
pnpm install -D vitest
```

### Issue: Import errors in IDE

**Solution:** TypeScript may need to rebuild. Restart the TypeScript server or rebuild:

```bash
pnpm build
```

### Issue: No deduplication happening in runtime

**Check:**

1. Message count >= 10? (Early exit if below)
2. Are the tool calls truly identical? (Same arguments AND response)
3. Are duplicates in the compressible range? (Not in last 3 messages)
4. Check debug logs for deduplication stats

### Issue: "any" type errors

**Solution:** The implementation uses proper TypeScript types. Ensure:

```typescript
import { Message } from '@/models/chat';
```

All functions have proper type signatures (no `any` used).

## Success Indicators

✅ **All tests pass**
✅ **No type errors**
✅ **No lint errors**
✅ **Debug logging shows deduplication stats**
✅ **Message count reduced in runtime**
✅ **Metadata added to deduplicated messages**
✅ **Content shows "(repeated Nx)" indicator**
✅ **Performance < 20ms for large chats**
✅ **No impact on small chats (early exit)**

## Next Steps

Once verified, the implementation is ready for:

1. **Code review** - Review with team
2. **Integration testing** - Test with real AI workflows
3. **Performance monitoring** - Track token savings in production
4. **Phase 2 planning** - Begin thinking block compression

## Contact

For questions or issues with this implementation, refer to:

- `IMPLEMENTATION_SUMMARY_DEDUPLICATION.md` - Overview and architecture
- `DEDUPLICATION_VISUAL_GUIDE.md` - Visual flow and examples
- `src/lib/message-deduplicator.test.ts` - Test cases and examples
