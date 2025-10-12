# OpenAI Tool Message Error Analysis

**Date**: 2025-10-12  
**Error**: `400 Invalid parameter: messages with role 'tool' must be a response to a preceeding message with 'tool_calls'.`

## 🔴 Problem Summary

OpenAI API is rejecting message stacks because **tool messages exist without their corresponding assistant messages with tool_calls**. This error does NOT occur in Gemini because Gemini has aggressive message stack validation that removes orphaned tool messages.

## 🔍 Root Cause Analysis

### 1. OpenAI's Strict Message Ordering Requirements

OpenAI API enforces strict message ordering rules:

- Every `tool` message **MUST** be preceded by an `assistant` message with `tool_calls`
- The `tool_call_id` in the tool message must match one of the `tool_calls` IDs in the preceding assistant message
- If this pairing is broken, the API returns a 400 error

### 2. Message Stack Processing Differences

#### OpenAI (`openai.ts`)

```typescript
async *streamChat(messages: Message[], options) {
  // Step 1: Base sanitization (removes thinking fields, converts tool_use to tool_calls)
  const { sanitizedMessages } = this.prepareStreamChat(messages, options);

  // Step 2: Convert to OpenAI format
  const openaiMessages = this.convertToOpenAIMessages(
    sanitizedMessages,
    options.systemPrompt,
  );

  // Step 3: Send to OpenAI API ❌ NO VALIDATION OF TOOL CALL CHAINS
}

private convertToOpenAIMessages(messages: Message[], systemPrompt?: string) {
  // Simply converts messages to OpenAI format
  // ❌ Does NOT validate or fix orphaned tool messages
  for (const m of messages) {
    if (effectiveRole === 'tool') {
      if (m.tool_call_id) {
        openaiMessages.push({
          role: 'tool',
          tool_call_id: m.tool_call_id,
          content: this.processMessageContent(m.content),
        });
      } else {
        logger.warn(`Tool message missing tool_call_id: ${JSON.stringify(m)}`);
      }
    }
  }
  return openaiMessages;
}
```

**Problem**: OpenAI implementation does NOT validate whether:

- A tool message has a corresponding assistant message with tool_calls
- The tool_call_id matches any existing tool_calls
- The message order is valid

#### Gemini (`gemini.ts`)

```typescript
async *streamChat(messages: Message[], options) {
  // Step 1: Base sanitization
  const { config, tools } = this.prepareStreamChat(messages, options);

  // Step 2: ✅ VALIDATE MESSAGE STACK FOR GEMINI
  const validatedMessages = this.validateGeminiMessageStack(messages);

  // Step 3: Convert to Gemini format
  const geminiMessages = this.convertToGeminiMessages(validatedMessages);
}

private validateGeminiMessageStack(messages: Message[]): Message[] {
  // ✅ Converts ALL tool messages to user messages
  const convertedMessages = messages.map((m) => {
    if (m.role === 'tool') {
      return { ...m, role: 'user' as const };
    }
    return m;
  });

  // ✅ Removes messages before first user message
  const firstUserIndex = convertedMessages.findIndex(
    (msg) => msg.role === 'user',
  );
  const validMessages = convertedMessages.slice(firstUserIndex);

  return validMessages;
}
```

**Solution**: Gemini's validation:

1. Converts **ALL** `tool` role messages to `user` role
2. Removes any messages before the first user message
3. This eliminates the possibility of orphaned tool messages

### 3. MessageNormalizer's Limited Scope

The `MessageNormalizer.sanitizeMessagesForProvider()` handles:

- ✅ Converting `thinking` fields removal for OpenAI family
- ✅ Converting `tool_use` ↔️ `tool_calls` format conversion
- ✅ **For Anthropic ONLY**: `fixAnthropicToolCallChain()` validates tool call pairing

But for OpenAI family:

```typescript
private static sanitizeForOpenAIFamily(message: Message): Message {
  // Remove thinking fields
  if (message.thinking) delete message.thinking;
  if (message.thinkingSignature) delete message.thinkingSignature;

  // Convert tool_use to tool_calls
  if (message.tool_use && !message.tool_calls) {
    message.tool_calls = [...];
    delete message.tool_use;
  }

  // ❌ NO VALIDATION OF TOOL CALL CHAINS
  return message;
}
```

## 🐛 How Orphaned Tool Messages Can Occur

### Scenario 1: Message Stack Manipulation

```typescript
// Original stack:
[
  { role: 'assistant', tool_calls: [{ id: 'call_1', ... }] },  // Message A
  { role: 'tool', tool_call_id: 'call_1', ... },              // Message B
  { role: 'user', ... },                                       // Message C
]

// After some UI operation (e.g., delete Message A):
[
  { role: 'tool', tool_call_id: 'call_1', ... },  // ❌ Orphaned!
  { role: 'user', ... },
]
```

### Scenario 2: Incomplete Tool Call Response

```typescript
// Assistant makes tool call
{ role: 'assistant', tool_calls: [{ id: 'call_1' }, { id: 'call_2' }] }

// Only one tool responds (network error, timeout, etc.)
{ role: 'tool', tool_call_id: 'call_1', ... }

// Missing: { role: 'tool', tool_call_id: 'call_2', ... }
```

### Scenario 3: Cross-Provider Message Stack Reuse

```typescript
// Messages created with Anthropic (uses tool_use format)
const anthropicMessages = [
  { role: 'assistant', tool_use: { id: 'call_1', ... } },
  { role: 'tool', tool_call_id: 'call_1', ... },
];

// Converted by MessageNormalizer for OpenAI
// tool_use → tool_calls conversion might fail
// Result: tool message without matching tool_calls
```

## 📊 Comparison Table

| Feature                        | OpenAI       | Gemini                    | Anthropic                    |
| ------------------------------ | ------------ | ------------------------- | ---------------------------- |
| Tool message validation        | ❌ None      | ✅ Converts to user       | ✅ fixAnthropicToolCallChain |
| Orphaned tool message handling | ❌ API Error | ✅ Converts to user       | ✅ Removes incomplete chains |
| Message order validation       | ❌ None      | ✅ Starts from first user | ✅ Validates tool pairs      |
| Tool call chain validation     | ❌ None      | N/A (no tool role)        | ✅ Validates completion      |

## 🔧 Recommended Solutions

### Solution A: Add OpenAI-Specific Validation (Minimal Impact)

Add a validation method similar to Gemini's `validateGeminiMessageStack`:

```typescript
// In openai.ts
private validateOpenAIMessageStack(messages: Message[]): Message[] {
  const result: Message[] = [];
  const assistantToolCallIds = new Set<string>();

  // First pass: collect all tool_call IDs from assistant messages
  for (const msg of messages) {
    if (msg.role === 'assistant' && msg.tool_calls) {
      msg.tool_calls.forEach(tc => assistantToolCallIds.add(tc.id));
    }
  }

  // Second pass: filter out orphaned tool messages
  for (const msg of messages) {
    if (msg.role === 'tool') {
      if (msg.tool_call_id && assistantToolCallIds.has(msg.tool_call_id)) {
        result.push(msg);
      } else {
        logger.warn('Skipping orphaned tool message', {
          messageId: msg.id,
          toolCallId: msg.tool_call_id,
        });
      }
    } else {
      result.push(msg);
    }
  }

  return result;
}
```

### Solution B: Extend MessageNormalizer (Consistent Approach)

Add OpenAI tool chain validation to `MessageNormalizer`:

```typescript
// In message-normalizer.ts
static sanitizeMessagesForProvider(
  messages: Message[],
  targetProvider: AIServiceProvider,
): Message[] {
  let processedMessages = messages;

  // Apply provider-specific chain validation
  if (targetProvider === AIServiceProvider.Anthropic) {
    processedMessages = this.fixAnthropicToolCallChain(messages);
  } else if (
    targetProvider === AIServiceProvider.OpenAI ||
    targetProvider === AIServiceProvider.Groq ||
    targetProvider === AIServiceProvider.Cerebras ||
    targetProvider === AIServiceProvider.Fireworks
  ) {
    // ✅ Add OpenAI family validation
    processedMessages = this.fixOpenAIFamilyToolCallChain(messages);
  }

  // Continue with existing sanitization
  return processedMessages
    .map((msg) => this.sanitizeSingleMessage(msg, targetProvider))
    .filter((msg) => msg !== null) as Message[];
}

private static fixOpenAIFamilyToolCallChain(messages: Message[]): Message[] {
  const result: Message[] = [];
  const toolCallIds = new Set<string>();

  // Collect valid tool_call IDs
  for (const msg of messages) {
    if (msg.role === 'assistant' && msg.tool_calls) {
      msg.tool_calls.forEach(tc => toolCallIds.add(tc.id));
    }
  }

  // Filter messages
  for (const msg of messages) {
    if (msg.role === 'tool') {
      if (!msg.tool_call_id || !toolCallIds.has(msg.tool_call_id)) {
        logger.warn('Removing orphaned tool message for OpenAI family', {
          messageId: msg.id,
          toolCallId: msg.tool_call_id,
        });
        continue; // Skip this message
      }
    }
    result.push(msg);
  }

  return result;
}
```

### Solution C: Sequence Validation (Most Robust)

Validate the actual sequence order, not just the presence of IDs:

```typescript
private static fixOpenAIFamilyToolCallChain(messages: Message[]): Message[] {
  const result: Message[] = [];
  const pendingToolCalls = new Map<string, boolean>();

  for (const msg of messages) {
    if (msg.role === 'assistant') {
      // Clear any pending tool calls from previous assistant message
      pendingToolCalls.clear();

      // Add new tool calls
      if (msg.tool_calls) {
        msg.tool_calls.forEach(tc => pendingToolCalls.set(tc.id, false));
      }
      result.push(msg);

    } else if (msg.role === 'tool') {
      // Only include if we have a pending tool call for this ID
      if (msg.tool_call_id && pendingToolCalls.has(msg.tool_call_id)) {
        pendingToolCalls.set(msg.tool_call_id, true);
        result.push(msg);
      } else {
        logger.warn('Skipping out-of-sequence tool message', {
          messageId: msg.id,
          toolCallId: msg.tool_call_id,
        });
      }

    } else {
      // User or system messages
      result.push(msg);
    }
  }

  return result;
}
```

## 🎯 Recommended Solution: Solution B (Extend MessageNormalizer)

**Rationale**:

1. **Consistency**: Uses the same architecture as Anthropic's tool chain validation
2. **Centralized Logic**: All provider-specific validation in one place
3. **Maintainability**: Easy to understand and debug
4. **Reusability**: Benefits all OpenAI-compatible providers (Groq, Cerebras, Fireworks)
5. **Non-Breaking**: Fits into existing code flow without changing interfaces

## 📝 Implementation Checklist

- [ ] Add `fixOpenAIFamilyToolCallChain()` method to `MessageNormalizer`
- [ ] Update `sanitizeMessagesForProvider()` to call it for OpenAI family
- [ ] Add comprehensive logging for debugging
- [ ] Write unit tests for edge cases:
  - [ ] Orphaned tool messages (no matching tool_call)
  - [ ] Out-of-sequence tool messages
  - [ ] Multiple assistant messages with tool_calls
  - [ ] Partial tool responses (some tool_calls missing responses)
- [ ] Test with all OpenAI-compatible providers:
  - [ ] OpenAI
  - [ ] Groq
  - [ ] Cerebras
  - [ ] Fireworks
- [ ] Document the validation behavior in code comments

## 🧪 Test Cases to Verify

```typescript
// Test Case 1: Orphaned tool message
const messages1 = [
  { role: 'tool', tool_call_id: 'call_1', content: 'Result' },
  { role: 'user', content: 'Hello' },
];
// Expected: Tool message removed

// Test Case 2: Valid tool chain
const messages2 = [
  { role: 'assistant', tool_calls: [{ id: 'call_1' }] },
  { role: 'tool', tool_call_id: 'call_1', content: 'Result' },
  { role: 'user', content: 'Hello' },
];
// Expected: All messages preserved

// Test Case 3: Partial tool responses
const messages3 = [
  { role: 'assistant', tool_calls: [{ id: 'call_1' }, { id: 'call_2' }] },
  { role: 'tool', tool_call_id: 'call_1', content: 'Result 1' },
  // Missing: call_2 response
  { role: 'user', content: 'Hello' },
];
// Expected: All messages preserved (OpenAI handles partial responses)

// Test Case 4: Out-of-sequence tool message
const messages4 = [
  { role: 'assistant', tool_calls: [{ id: 'call_1' }] },
  { role: 'user', content: 'Interrupt' },
  { role: 'tool', tool_call_id: 'call_1', content: 'Result' },
];
// Expected: Tool message removed (comes after user interruption)
```

## 🚨 Prevention Strategies

1. **Message Stack Integrity**: Implement validation when messages are created/deleted in the UI
2. **Transaction-Based Updates**: Ensure tool_calls and tool responses are atomic operations
3. **Provider-Agnostic Storage**: Store messages in a normalized format, convert only at API call time
4. **Error Recovery**: Implement retry logic that automatically fixes message stacks on 400 errors

## 📚 Related Files

- `/src/lib/ai-service/openai.ts` - OpenAI service implementation
- `/src/lib/ai-service/gemini.ts` - Gemini service with validation
- `/src/lib/ai-service/message-normalizer.ts` - Message sanitization logic
- `/src/lib/ai-service/base-service.ts` - Base service with prepareStreamChat
- `/src/models/chat.ts` - Message type definitions

## 🔗 OpenAI API Documentation

- [Chat Completions API](https://platform.openai.com/docs/api-reference/chat/create)
- [Function Calling Guide](https://platform.openai.com/docs/guides/function-calling)
- Error code 400: "messages with role 'tool' must be a response to a preceeding message with 'tool_calls'"
