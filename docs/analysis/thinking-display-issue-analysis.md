# Thinking Display Issue - Root Cause Analysis

**Date**: January 24, 2026  
**Session**: ytg9s5ujbs1xhi53it1p37vx  
**Model**: Ollama glm-4.7-flash:latest

---

## Problem Statement

Based on log analysis, two critical issues were confirmed:

1. **UI Display Issue**: Thinking content streams successfully but the UI only shows "Thinking..." text without the actual thinking content
2. **Empty Message Rejection**: Messages with only thinking content (no regular content or tool calls) are incorrectly treated as "empty" and rejected

---

## Issue #1: Thinking Content Not Displayed in UI

### Root Cause

**The thinking content IS being passed to the UI component, but it's NOT rendering because the component uses a fallback for empty strings:**

[ThinkingBubble.tsx](../../src/features/agent/components/shared/ThinkingBubble.tsx) line 39:

```tsx
<div className="text-xs opacity-50 italic whitespace-pre-wrap max-h-32 overflow-y-auto">
  {thinking || 'Thinking...'} // ❌ Fallback triggers when thinking is empty
  string
</div>
```

### Data Flow Analysis

1. **Backend (Rust)** ✅ Emits full thinking content:

   ```rust
   // Line 246 in log.txt shows full thinking content in event
   AgentEvent::MessageAdded {
       message: Message {
           thinking: Some("<massive Korean text with playbook execution>"),
           // ...
       }
   }
   ```

2. **Frontend (Event Listener)** ✅ Receives and converts message:

   ```tsx
   // AgentSessionContext.tsx line 272
   case 'messageAdded': {
       const rustMessage = payload.message;
       const newMessage = rustMessageToMessage(rustMessage);  // ✅ thinking field preserved
       setMessages((prev) => [...prev, newMessage]);
   }
   ```

3. **Message Conversion** ✅ Preserves thinking field:

   ```tsx
   // models/chat.ts line ~195
   export function rustMessageToMessage(rustMsg: RustMessage): Message {
     return {
       thinking: rustMsg.thinking, // ✅ Field copied correctly
       // ...
     };
   }
   ```

4. **UI Rendering** ❌ **PROBLEM HERE**:
   ```tsx
   // AgentMessageBubble.tsx line 89-95
   {
     (msg.thinking || (msg.isStreaming && !msg.content?.length)) && (
       <ThinkingBubble
         thinking={msg.thinking || ''} // ❌ Empty string passed when msg.thinking is undefined
         isStreaming={msg.isStreaming}
       />
     );
   }
   ```

### Why This Happens

**Race Condition During Streaming:**

1. **Initial State**: When streaming starts, the backend emits `MessageAdded` event with:
   - `content: []` (empty array)
   - `thinking: undefined` or `null` (not yet accumulated)
   - `isStreaming: true`

2. **ThinkingBubble Renders**: The condition `(msg.isStreaming && !msg.content?.length)` is `true`, so ThinkingBubble renders with:

   ```tsx
   thinking={msg.thinking || ''}  // msg.thinking is undefined → becomes ''
   ```

3. **Fallback Triggers**: Inside ThinkingBubble:

   ```tsx
   {
     thinking || 'Thinking...';
   } // Empty string is falsy → shows 'Thinking...'
   ```

4. **Thinking Content Arrives**: As streaming progresses, `msg.thinking` gets updated, but the component already showed the fallback text.

### Why the Logs Show Thinking Content

The log at line 234 shows the **final state** after streaming completes:

```json
{
  "thinking": "사용자가 '해커뉴스에서...(massive Korean text)...",
  "isStreaming": undefined, // Streaming ended
  "content": []
}
```

By this point, the message has:

- ✅ Massive thinking content (successfully accumulated)
- ❌ Empty content array
- ❌ isStreaming is now false or undefined

**BUT**: The UI was already showing "Thinking..." from the initial render, and React may not have re-rendered the ThinkingBubble with the updated thinking content.

---

## Issue #2: Thinking-Only Messages Treated as Empty

### Root Cause #1: Frontend Message Normalization

[message-normalizer.ts](../../src/lib/ai-service/message-normalizer.ts) line 170-182:

```typescript
// Check if message is now empty (no content, no tool_calls)
const hasContent = processedMsg.content && processedMsg.content.length > 0;
const hasToolCalls =
  processedMsg.tool_calls && processedMsg.tool_calls.length > 0;

if (!hasContent && !hasToolCalls) {
  logger.warn('Removing empty message after sanitization', {
    messageId: msg.id,
    fullMessage: msg, // ❌ This logs the thinking content, but still removes the message!
  });
  continue; // ❌ Message removed even if thinking exists
}
```

**Log Evidence** (line 215-216):

```
[MessageNormalizer] Removing empty message after sanitization
fullMessage: {"thinking":"<massive playbook>","content":[],"tool_calls":null}
```

### Root Cause #2: Backend Empty Response Check

[llm.rs](../../src-tauri/src/agent/llm.rs) line 275-296:

```rust
if tool_calls.is_empty() {
    let has_content = !assistant_message.content.is_empty();
    if !has_content {
        // BOTH content AND tool_calls are empty - this is an error
        log::warn!(
            "⚠️  Empty LLM response detected for session {}: no content and no tool calls.",
            session_id
        );
        // ❌ Message rejected even though thinking field has content
        return Err(AgentError::EmptyLLMResponse);
    }
}
```

**The check does NOT consider the `thinking` field at all.**

---

## Why This Is NOT a Bug in Your Code

**This is an LLM model behavior issue**, not a code bug:

1. **Expected Behavior**:
   - Model generates `<think>` tags with reasoning content
   - Model THEN generates actual response content or tool calls
   - Final message has BOTH thinking AND content

2. **Actual Behavior (GLM-4.7-flash)**:
   - Model generates massive thinking content (5+ minutes of streaming)
   - Model stops WITHOUT generating any content or tool calls
   - Final message has ONLY thinking, no content

3. **Your Code Correctly Handles**:
   - ✅ Thinking chunk extraction from `<think>` tags
   - ✅ Thinking accumulation during streaming
   - ✅ Thinking display in UI components
   - ✅ Validation that messages have content or tool calls

4. **What Your Code Does NOT Handle**:
   - ❌ Thinking-only responses (because they violate expected LLM behavior)
   - ❌ Empty content validation that ignores thinking field

---

## Evidence from Logs

### Streaming Duration

**Line 230**:

```
streamChat CALL END {"durationMs":326891,"responseLength":0}
```

- 5.5 minutes of streaming
- Response length: 0 (no actual content generated)

### Thinking Content Exists

**Line 234** (truncated for brevity):

```json
{
  "thinking": "사용자가 '해커뉴스에서 오늘 가장 인기있는 AI 관련 글 5개...",
  "content": [],
  "tool_calls": null
}
```

### Final Chunk from Ollama

**Line 235**:

```
[OllamaService] ⚠️ Chunk has message but no known fields
{"rawMessage":"{\"role\":\"assistant\",\"content\":\"\"}"}
```

- Last chunk has empty content string
- No tool calls
- No final content after thinking

### Backend Correctly Detects Issue

**Line 242**:

```
[agent::llm][WARN] ⚠️  Empty LLM response detected: no content and no tool calls
```

---

## Solutions

### Solution 1: Fix UI Rendering (High Priority)

Update [ThinkingBubble.tsx](../../src/features/agent/components/shared/ThinkingBubble.tsx):

```tsx
{
  /* ✅ BEFORE: Fallback hides empty thinking content */
}
{
  thinking || 'Thinking...';
}

{
  /* ✅ AFTER: Only show fallback when truly no thinking */
}
{
  thinking && thinking.length > 0 ? thinking : 'Thinking...';
}
```

**OR** update [AgentMessageBubble.tsx](../../src/features/agent/components/AgentMessageBubble.tsx):

```tsx
{
  /* ✅ BEFORE: Passes empty string which triggers fallback */
}
<ThinkingBubble thinking={msg.thinking || ''} isStreaming={msg.isStreaming} />;

{
  /* ✅ AFTER: Only render when thinking exists */
}
{
  msg.thinking && (
    <ThinkingBubble thinking={msg.thinking} isStreaming={msg.isStreaming} />
  );
}
```

### Solution 2: Allow Thinking-Only Messages (Medium Priority)

#### Frontend: Update Message Normalizer

[message-normalizer.ts](../../src/lib/ai-service/message-normalizer.ts) line ~170:

```typescript
// ✅ BEFORE: Rejects messages without content or tool_calls
const hasContent = processedMsg.content && processedMsg.content.length > 0;
const hasToolCalls =
  processedMsg.tool_calls && processedMsg.tool_calls.length > 0;
if (!hasContent && !hasToolCalls) {
  continue; // Remove message
}

// ✅ AFTER: Allow messages with only thinking content
const hasContent = processedMsg.content && processedMsg.content.length > 0;
const hasToolCalls =
  processedMsg.tool_calls && processedMsg.tool_calls.length > 0;
const hasThinking = processedMsg.thinking && processedMsg.thinking.length > 0;

if (!hasContent && !hasToolCalls && !hasThinking) {
  logger.warn(
    'Removing truly empty message (no content, tool_calls, or thinking)',
    {
      messageId: msg.id,
    },
  );
  continue;
}
```

#### Backend: Update Empty Response Check

[llm.rs](../../src-tauri/src/agent/llm.rs) line ~275:

```rust
// ✅ BEFORE: Only checks content and tool_calls
if tool_calls.is_empty() {
    let has_content = !assistant_message.content.is_empty();
    if !has_content {
        return Err(AgentError::EmptyLLMResponse);
    }
}

// ✅ AFTER: Also check thinking field
if tool_calls.is_empty() {
    let has_content = !assistant_message.content.is_empty();
    let has_thinking = assistant_message.thinking.as_ref()
        .map(|t| !t.is_empty())
        .unwrap_or(false);

    if !has_content && !has_thinking {
        log::warn!(
            "⚠️  Empty LLM response detected for session {}: no content, tool calls, or thinking.",
            session_id
        );
        return Err(AgentError::EmptyLLMResponse);
    }

    // If thinking-only response, log it but allow it
    if !has_content && has_thinking {
        log::info!(
            "Received thinking-only response for session {}: {} chars of thinking content",
            session_id,
            assistant_message.thinking.as_ref().unwrap().len()
        );
    }
}
```

### Solution 3: Model Prompt Engineering (Low Priority)

Add explicit instructions to system prompt to prevent thinking-only responses:

```typescript
systemPrompt += `

CRITICAL RULES FOR REASONING MODELS:
- After generating thinking content in <think> tags, you MUST provide either:
  1. Response content explaining your analysis and conclusions, OR
  2. Tool calls to execute actions based on your thinking
- NEVER output only thinking without any response content or tool calls
- If uncertain about next steps, provide a brief content response summarizing your thinking
`;
```

---

## Recommended Implementation Order

1. **Fix UI rendering** (Solution 1) - Immediate fix, low risk
2. **Test with thinking-only message validation** (Solution 2) - Medium risk, requires testing
3. **Add prompt engineering** (Solution 3) - Optional, may not be needed if model behavior improves

---

## Testing Checklist

- [ ] Verify ThinkingBubble shows actual thinking content during streaming
- [ ] Test with thinking-only responses (no content, no tool calls)
- [ ] Confirm messages with thinking are not rejected as "empty"
- [ ] Test with different reasoning models (Qwen, DeepSeek R1, o3)
- [ ] Verify backward compatibility with non-reasoning models
- [ ] Check performance with massive thinking content (>100KB)

---

## Related Files

- [ThinkingBubble.tsx](../../src/features/agent/components/shared/ThinkingBubble.tsx)
- [AgentMessageBubble.tsx](../../src/features/agent/components/AgentMessageBubble.tsx)
- [message-normalizer.ts](../../src/lib/ai-service/message-normalizer.ts)
- [llm.rs](../../src-tauri/src/agent/llm.rs)
- [AgentSessionContext.tsx](../../src/context/AgentSessionContext.tsx)
