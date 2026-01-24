# Fix: Thinking Content Not Displaying During Streaming

**Issue Date**: January 24, 2026  
**Status**: ✅ FIXED

---

## Problem

During Agent V2 streaming, thinking content was not displayed in the UI despite being successfully accumulated. The UI only showed "Thinking..." placeholder text instead of the actual thinking content.

---

## Root Cause

**Race Condition in Message State Management**:

1. Backend emits `messageAdded` event with partial message:

   ```json
   {
     "id": "msg_xxx",
     "thinking": "some initial thinking",
     "content": [],
     "isStreaming": true
   }
   ```

2. Frontend `AgentSessionContext` adds this message to `sessionMessages` state

3. Meanwhile, `LLMServiceContext` accumulates thinking chunks:

   ```tsx
   streamingMessages.set(sessionId, {
     ...message,
     thinking: accumulatedThinkingContent, // Growing content
     isStreaming: true,
   });
   ```

4. **Problem**: `AgentChatContext.displayMessages` checks if message exists:

   ```tsx
   const existsInMessages = sessionMessages.some(
     (m) => m.id === currentStreamingMessage.id,
   );
   if (!existsInMessages) {
     // Only add if doesn't exist
   }
   ```

5. **Result**: Since message already exists in `sessionMessages` (from step 2), the streaming version with accumulated thinking is **ignored**!

---

## Solution

### 1. **Prioritize Streaming Message** ([AgentChatContext.tsx](../../src/context/AgentChatContext.tsx))

Changed message merge logic to **replace** existing message with streaming version:

```tsx
// ✅ BEFORE: Only added streaming message if it didn't exist
const existsInMessages = sessionMessages.some(
  (m) => m.id === currentStreamingMessage.id,
);
if (!existsInMessages) {
  return [...sessionMessages, currentStreamingMessage as Message];
}

// ✅ AFTER: Replace existing message with streaming version
const existingIndex = sessionMessages.findIndex(
  (m) => m.id === currentStreamingMessage.id,
);

if (existingIndex >= 0) {
  // Replace with streaming version (has accumulated thinking/content)
  const updated = [...sessionMessages];
  updated[existingIndex] = currentStreamingMessage as Message;
  return updated;
} else {
  // Append if new
  return [...sessionMessages, currentStreamingMessage as Message];
}
```

**Why This Works**: Streaming message in `LLMServiceContext.streamingMessages` always has the latest accumulated thinking content.

---

### 2. **Fix Empty String Handling** ([AgentMessageBubble.tsx](../../src/features/agent/components/AgentMessageBubble.tsx))

Changed from passing empty string fallback to passing `undefined`:

```tsx
// ✅ BEFORE: Empty string triggers fallback
<ThinkingBubble
  thinking={msg.thinking || ''}  // ❌ '' is falsy
  isStreaming={msg.isStreaming}
/>

// ✅ AFTER: Pass undefined if no thinking
<ThinkingBubble
  thinking={msg.thinking}  // ✅ undefined or actual content
  isStreaming={msg.isStreaming}
/>
```

---

### 3. **Improve Fallback Logic** ([ThinkingBubble.tsx](../../src/features/agent/components/shared/ThinkingBubble.tsx))

Changed fallback condition to only show when truly no content:

```tsx
// ✅ BEFORE: Empty string shows fallback
{
  thinking || 'Thinking...';
} // ❌ '' is falsy

// ✅ AFTER: Only show fallback for null/undefined or empty
{
  thinking != null && thinking.length > 0 ? thinking : 'Thinking...';
}
```

---

## Testing

### Before Fix

- UI shows: "Thinking..." (static text)
- Console shows: Thinking content being accumulated in `streamingMessages`
- Backend logs: Full thinking content in message

### After Fix

- UI shows: Actual thinking content (live updates)
- Thinking bubble updates as chunks arrive
- Final message preserves all thinking content

---

## Files Changed

1. [src/context/AgentChatContext.tsx](../../src/context/AgentChatContext.tsx) - Message merge logic
2. [src/features/agent/components/AgentMessageBubble.tsx](../../src/features/agent/components/AgentMessageBubble.tsx) - Prop passing
3. [src/features/agent/components/shared/ThinkingBubble.tsx](../../src/features/agent/components/shared/ThinkingBubble.tsx) - Fallback condition

---

## Related Issues

This fix also resolves the foundation for thinking-only message validation (Issue #2 in [thinking-display-issue-analysis.md](../analysis/thinking-display-issue-analysis.md)).

---

## Future Improvements

Consider consolidating message state management to avoid race conditions between:

- `AgentSessionContext.sessionMessages` (persisted from backend events)
- `LLMServiceContext.streamingMessages` (real-time streaming state)
