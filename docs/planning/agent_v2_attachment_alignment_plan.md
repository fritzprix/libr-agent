# Agent V2 File Attachment Alignment Plan

## Executive Summary

**Goal**: Align Agent V2 file attachment feature with V1 Chat implementation to ensure:

1. LLM receives attachment metadata and tool usage instructions
2. Users can see attachment history in message bubbles

**Status**: Agent V2 has 70% feature parity - infrastructure is complete, but two critical integration points are missing.

---

## Current State Analysis

### ✅ What's Working (Already Aligned)

| Component       | V1 Chat                     | V2 Agent                    | Status        |
| --------------- | --------------------------- | --------------------------- | ------------- |
| File Collection | `useFileAttachment.ts`      | `useAgentFileAttachment.ts` | ✅ Identical  |
| Storage Layer   | `ResourceAttachmentContext` | Same (shared)               | ✅ Reused     |
| Message Schema  | `Message.attachments`       | Same field                  | ✅ Compatible |
| Backend Storage | content-store server        | Same server                 | ✅ Shared     |

### ❌ What's Missing (Needs Implementation)

| Feature               | V1 Chat                            | V2 Agent           | Impact                     |
| --------------------- | ---------------------------------- | ------------------ | -------------------------- |
| **LLM Preprocessing** | `prepareMessagesForLLM()`          | ❌ Not called      | AI cannot see/access files |
| **Visual Display**    | `MessageBubble` attachment section | ❌ Not implemented | Users can't see history    |

---

## Problem Statement

### Issue #1: LLM Never Sees Attachments

**Current Flow:**

```
User attaches file → Stored in content-store → Message.attachments populated
→ LLMServiceContext sends message → AI receives raw message → No file info!
```

**Expected Flow:**

```
User attaches file → Stored in content-store → Message.attachments populated
→ prepareMessagesForLLM() enriches content → AI receives metadata + tool instructions
```

**Evidence:**

- `LLMServiceContext.tsx` line ~340: `streamChat(safeMessages, ...)` called directly
- No import or call to `prepareMessagesForLLM()`
- Messages sent to AI without attachment enrichment

**Impact:**

- AI has no knowledge of attached files
- AI cannot use content-store tools to read files
- User expectation broken: "I attached a file but AI ignores it"

---

### Issue #2: Users Can't See Attachment History

**Current Behavior:**

- `AgentChatAttachedFiles.tsx` only shows **pending** files (before commit)
- Once message sent, attachments disappear from UI
- Message history shows no indication files were attached

**Evidence:**

- `AgentMessageBubble.tsx`: No attachment display logic
- `AgentChatAttachedFiles.tsx` line 9: `attachedFiles = pendingFiles` (only pre-commit)

**Impact:**

- Users lose context: "Which message had that PDF?"
- Cannot verify files were actually attached
- Poor UX compared to V1 Chat

---

## Refactoring Plan

### Phase 1: Add LLM Preprocessing (Critical)

**Priority**: 🔴 **HIGH** - Blocks core functionality

#### Step 1.1: Import Preprocessor in LLMServiceContext

**File**: `src/context/LLMServiceContext.tsx`

**Change**:

```typescript
// Add import at top of file
import { prepareMessagesForLLM } from '@/lib/message-preprocessor';
```

**Rationale**: Reuse existing V1 Chat preprocessor - it's already battle-tested.

---

#### Step 1.2: Add Preprocessing Before LLM Call

**File**: `src/context/LLMServiceContext.tsx`

**Location**: Inside `executeCompletionRequest` function, around line 340

**Current Code**:

```typescript
// Sanitize messages to prevent malformed JSON and ensure provider compatibility
const safeMessages = MessageNormalizer.sanitizeMessagesForProvider(
  contextMessages.map(sanitizeMessage),
  provider as unknown as AIServiceProvider,
);
logger.info('✅ Messages sanitized for provider compatibility', {
  sessionId,
  originalCount: contextMessages.length,
  safeCount: safeMessages.length,
});

// ... token estimation logs ...

// CREATE ASYNC GENERATOR FOR STREAMING
const streamGenerator = service.streamChat(safeMessages, {
  modelName: model,
  systemPrompt: finalSystemPrompt,
  availableTools: availableTools || [],
  config,
  forceToolUse: false,
});
```

**New Code**:

```typescript
// Sanitize messages to prevent malformed JSON and ensure provider compatibility
const safeMessages = MessageNormalizer.sanitizeMessagesForProvider(
  contextMessages.map(sanitizeMessage),
  provider as unknown as AIServiceProvider,
);
logger.info('✅ Messages sanitized for provider compatibility', {
  sessionId,
  originalCount: contextMessages.length,
  safeCount: safeMessages.length,
});

// Preprocess messages to include attachment information
// This enriches messages with attachment metadata and tool usage instructions
const enrichedMessages = await prepareMessagesForLLM(safeMessages);

// Log attachment enrichment
const attachmentCount = enrichedMessages.reduce(
  (total, msg) => total + (msg.attachments?.length || 0),
  0,
);
if (attachmentCount > 0) {
  logger.info('📎 Messages enriched with attachment metadata', {
    sessionId,
    attachmentCount,
    messagesWithAttachments: enrichedMessages.filter(
      (m) => m.attachments && m.attachments.length > 0,
    ).length,
  });
}

// ... token estimation logs ...

// CREATE ASYNC GENERATOR FOR STREAMING
const streamGenerator = service.streamChat(enrichedMessages, {
  modelName: model,
  systemPrompt: finalSystemPrompt,
  availableTools: availableTools || [],
  config,
  forceToolUse: false,
});
```

**Key Changes**:

1. Add `prepareMessagesForLLM()` call after sanitization
2. Use `enrichedMessages` instead of `safeMessages` for streaming
3. Add logging for attachment enrichment (debugging visibility)

**Why This Works**:

- `prepareMessagesForLLM()` adds attachment metadata to `message.content` array
- LLM sees attachment info as text content (not separate field)
- Instructions tell AI how to use content-store tools to read files

---

#### Step 1.3: Update Token Estimation (Optional but Recommended)

**Location**: After `prepareMessagesForLLM()` call

**Current Code**:

```typescript
// Measure final token count for logging
const totalEstimatedTokens = safeMessages.reduce(
  (sum, msg) => sum + estimateTokensBPE(msg),
  0,
);
```

**New Code**:

```typescript
// Measure final token count for logging (including attachment enrichment)
const totalEstimatedTokens = enrichedMessages.reduce(
  (sum, msg) => sum + estimateTokensBPE(msg),
  0,
);
```

**Rationale**: Token estimate should include enriched content for accuracy.

---

### Phase 2: Add Visual Attachment Display (Important)

**Priority**: 🟡 **MEDIUM** - UX improvement

#### Step 2.1: Add Attachment Display Section

**File**: `src/features/agent/components/AgentMessageBubble.tsx`

**Location**: Inside the message bubble, before content rendering (around line 86)

**Current Code**:

```tsx
<div className="whitespace-pre-wrap">
  {(msg.content && msg.content.length > 0) ||
  msg.thinking ||
  msg.isStreaming ? (
    <>
      {/* Thinking bubble */}
      {/* ... */}

      {/* Actual content */}
      {msg.content && msg.content.length > 0 && (
        <AgentMessageRenderer content={msg.content} message={msg} />
      )}
    </>
  ) : (
    <span className="text-muted-foreground italic">No content</span>
  )}
</div>
```

**New Code**:

```tsx
<div className="whitespace-pre-wrap">
  {/* File Attachments Display */}
  {msg.attachments && msg.attachments.length > 0 && (
    <div className="mb-3 p-3 bg-muted/30 rounded-lg border border-muted/20">
      <div className="text-sm mb-2 font-medium flex items-center gap-2">
        <span>📎</span>
        <span>
          {msg.attachments.length} file
          {msg.attachments.length > 1 ? 's' : ''} attached
        </span>
      </div>
      <div className="space-y-2">
        {msg.attachments.map((attachment) => (
          <div
            key={attachment.contentId}
            className="flex items-center justify-between p-2 bg-background/50 rounded border"
          >
            <div className="flex items-center gap-2 min-w-0 flex-1">
              <span className="text-xs">📄</span>
              <span className="text-xs font-medium truncate">
                {attachment.filename}
              </span>
              <span className="text-xs opacity-60 whitespace-nowrap">
                ({Math.round(attachment.size / 1024)}KB)
              </span>
            </div>
            <div className="text-xs opacity-50 whitespace-nowrap ml-2">
              {attachment.lineCount} lines
            </div>
          </div>
        ))}
      </div>
    </div>
  )}

  {(msg.content && msg.content.length > 0) ||
  msg.thinking ||
  msg.isStreaming ? (
    <>
      {/* Thinking bubble */}
      {/* ... */}

      {/* Actual content */}
      {msg.content && msg.content.length > 0 && (
        <AgentMessageRenderer content={msg.content} message={msg} />
      )}
    </>
  ) : (
    <span className="text-muted-foreground italic">No content</span>
  )}
</div>
```

**Key Features**:

- Shows file count header with 📎 icon
- Lists each file with:
  - 📄 File icon
  - Filename (truncated if long)
  - Size in KB
  - Line count
- Styled to match V1 Chat design
- Appears above message content

---

#### Step 2.2: Add AttachmentReference Type Import

**File**: `src/features/agent/components/AgentMessageBubble.tsx`

**Location**: Top of file, add to imports

**Add**:

```tsx
import type { Message, AttachmentReference } from '@/models/chat';
```

**Note**: This ensures TypeScript recognizes `attachment.contentId`, `attachment.filename`, etc.

---

### Phase 3: Testing & Validation

#### Test Case 1: File Upload & LLM Recognition

**Steps**:

1. Start new agent session
2. Attach a `.txt` file with sample content
3. Send message: "What files do I have attached?"
4. Verify AI response mentions the file by name

**Expected Behavior**:

- AI should see attachment metadata in content
- AI should be able to use `readContent()` tool to read file
- AI should respond with filename and offer to read contents

**Debug Check**:

```typescript
// Check logs for:
'📎 Messages enriched with attachment metadata';
// Should show attachmentCount > 0
```

---

#### Test Case 2: Visual Display

**Steps**:

1. Send message with 2 files attached
2. Scroll up in chat history
3. Verify attachment section appears in message bubble

**Expected Behavior**:

- 📎 header shows "2 files attached"
- Both files listed with icons, names, sizes, line counts
- Styling matches V1 Chat (muted background, border)

**Visual Check**:

- Attachment section above message text
- Truncated long filenames with ellipsis
- Responsive layout (no overflow)

---

#### Test Case 3: Message Without Attachments

**Steps**:

1. Send regular text message (no files)
2. Verify no attachment section appears

**Expected Behavior**:

- No 📎 section rendered
- Normal message display
- No console errors

---

#### Test Case 4: Cross-Session Persistence

**Steps**:

1. Send message with file attachment
2. Refresh page
3. Open same session
4. Verify attachment still visible in history

**Expected Behavior**:

- Attachments persist across page reloads
- Message reloads with full attachment metadata
- No data loss

---

### Phase 4: Edge Cases & Error Handling

#### Edge Case 1: Large Attachment Metadata

**Scenario**: User attaches 10 files to one message

**Consideration**:

- `prepareMessagesForLLM()` adds metadata to content
- Large metadata may consume token budget
- May need to truncate preview or limit attachment count

**Solution** (Optional Enhancement):

```typescript
// In message-preprocessor.ts, limit attachment metadata
const MAX_ATTACHMENTS_IN_PROMPT = 5;
const attachmentsToShow = message.attachments.slice(
  0,
  MAX_ATTACHMENTS_IN_PROMPT,
);

if (message.attachments.length > MAX_ATTACHMENTS_IN_PROMPT) {
  // Add note: "... and N more files. Use listContent() to see all."
}
```

---

#### Edge Case 2: Missing Attachment Data

**Scenario**: Message has `attachments` field but data is incomplete

**Current Handling**:

- `AgentMessageBubble.tsx` will crash if `attachment.contentId` is undefined
- Need defensive checks

**Solution**:

```tsx
{msg.attachments?.filter(a => a.contentId && a.filename).map((attachment) => (
  // Render only valid attachments
))}
```

---

#### Edge Case 3: Attachment After Message Sent

**Scenario**: User tries to access file that was deleted from content-store

**Current Behavior**:

- Metadata shows in message, but file is gone
- AI tool call fails with "Content not found"

**Solution** (Future Enhancement):

- Add "file not found" indicator in UI
- Show strikethrough on missing files
- Requires content-store health check API

---

## Implementation Checklist

### Pre-Implementation

- [ ] Checkout new branch: `git checkout -b feat/agent-v2-attachment-alignment`
- [ ] Review current attachment flow in V1 Chat
- [ ] Verify `prepareMessagesForLLM()` is production-ready

### Phase 1: LLM Preprocessing

- [ ] Add import in `LLMServiceContext.tsx`
- [ ] Add `prepareMessagesForLLM()` call before streaming
- [ ] Update token estimation to use enriched messages
- [ ] Add logging for attachment enrichment
- [ ] Test: Verify logs show attachment count

### Phase 2: Visual Display

- [ ] Add attachment section in `AgentMessageBubble.tsx`
- [ ] Add `AttachmentReference` type import
- [ ] Test: Verify UI renders attachments
- [ ] Test: Verify no errors for messages without attachments

### Phase 3: Testing

- [ ] Run Test Case 1: File Upload & LLM Recognition
- [ ] Run Test Case 2: Visual Display
- [ ] Run Test Case 3: Message Without Attachments
- [ ] Run Test Case 4: Cross-Session Persistence

### Phase 4: Code Quality

- [ ] Run `pnpm lint` - verify no errors
- [ ] Run `pnpm format` - format code
- [ ] Run `pnpm build` - verify builds successfully
- [ ] Run `pnpm dead-code` - check for unused imports

### Phase 5: Documentation

- [ ] Update CHANGELOG.md with feature addition
- [ ] Add comments explaining preprocessing flow
- [ ] Document attachment display behavior

---

## Expected Outcomes

### After Phase 1 (LLM Preprocessing)

✅ AI can see attached files
✅ AI can use content-store tools to read files
✅ Logs show attachment enrichment
✅ Token budget accounts for attachment metadata

### After Phase 2 (Visual Display)

✅ Users see attachment history in message bubbles
✅ Attachment section styled like V1 Chat
✅ File metadata visible (name, size, lines)
✅ No UI errors or layout issues

### Overall Impact

✅ Agent V2 reaches 100% feature parity with V1 Chat attachments
✅ Users can attach files and AI understands them
✅ UX matches expectations from V1 Chat
✅ No breaking changes to existing functionality

---

## Rollback Plan

If issues occur during implementation:

1. **LLM Preprocessing Breaks Streaming**:
   - Revert `LLMServiceContext.tsx` changes
   - Use `safeMessages` instead of `enrichedMessages`
   - File attachment feature disabled but no crashes

2. **Visual Display Causes Render Errors**:
   - Wrap attachment section in `try-catch`
   - Log error and render fallback (no attachment display)
   - Core message rendering unaffected

3. **Token Budget Exceeded**:
   - Add conditional preprocessing:
     ```typescript
     const enrichedMessages = messages.some((m) => m.attachments?.length)
       ? await prepareMessagesForLLM(safeMessages)
       : safeMessages;
     ```

---

## Success Metrics

### Functional Metrics

- ✅ LLM receives attachment metadata in 100% of cases
- ✅ Visual display renders for 100% of messages with attachments
- ✅ Zero crashes or errors in production logs

### Performance Metrics

- ⏱️ Preprocessing adds <50ms latency to LLM request
- 📊 Token usage increase: <5% for typical attachments
- 🎨 UI render time: <16ms for attachment section

### User Satisfaction

- 👍 Users report AI successfully reads attached files
- 👍 Users can see attachment history in chat
- 👍 Feature parity with V1 Chat confirmed

---

## Future Enhancements (Post-MVP)

### Enhancement 1: Attachment Preview Modal

- Click file name to open preview modal
- Show first 100 lines of file content
- Add "Open in Workspace" button

### Enhancement 2: Inline File Actions

- Add download button next to each attachment
- Add "Ask AI about this file" quick action
- Add delete attachment button (if before message sent)

### Enhancement 3: Smart Attachment Suggestions

- Detect when AI needs a file and suggest upload
- Auto-suggest related files from workspace
- Show "Recently attached" file list

### Enhancement 4: Batch Attachment Management

- Select multiple messages and extract all attachments
- Export attachments as ZIP
- Copy attachment references to clipboard

---

## References

### Existing Code to Study

- V1 Chat Preprocessing: `src/lib/message-preprocessor.ts` lines 15-70
- V1 Chat Display: `src/features/chat/MessageBubble.tsx` lines 148-179
- Attachment Storage: `src/context/ResourceAttachmentContext.tsx`
- Type Definitions: `src/models/chat.ts` lines 19-40

### Related Documentation

- [Chat Feature Architecture](../architecture/chat-feature-architecture.md)
- [UI Resource Implementation Guide](../guides/ui-resource-implementation.md)
- [Attachment System Overview](../features/attachment-system.md)

### Testing Tools

- Manual testing: Use `pnpm tauri dev`
- Log inspection: Check console and Tauri logs
- Token counting: Use `estimateTokensBPE()` utility
- Visual regression: Compare with V1 Chat screenshots

---

## Conclusion

This refactoring plan addresses the 30% feature gap in Agent V2's file attachment support. Both missing components are straightforward to implement:

1. **LLM Preprocessing**: Add 10 lines of code in `LLMServiceContext.tsx`
2. **Visual Display**: Add 40 lines of JSX in `AgentMessageBubble.tsx`

Total effort: ~2-4 hours including testing.

Risk level: **LOW** - Both components are isolated and use existing, tested code patterns from V1 Chat.

**Recommended Timeline**:

- Phase 1 (LLM): 1 hour implementation + 1 hour testing
- Phase 2 (UI): 1 hour implementation + 1 hour testing
- Total: Half-day development sprint

Once complete, Agent V2 will have full feature parity with V1 Chat for file attachments, enabling users to seamlessly attach files and have AI process them using content-store MCP tools.
