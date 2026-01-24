# Sprint W3-Jan-26: Playbook Feature Implementation

## Overview

Implement a comprehensive Playbook feature that allows users to browse, bookmark, and execute saved workflows. When a user selects a playbook, the agent automatically loads the playbook context and begins execution without requiring manual commands.

---

## Feature Requirements

### 1. Playbook List Page

- **Grid Layout**: Display playbooks as cards in a responsive grid
- **Card Information**: Show goal, step count, creation date, and assistant name
- **Bookmarking**: Users can bookmark favorite playbooks
- **Sorting Options**:
  - Sort by creation date (newest/oldest first)
  - Sort by assistant name (A-Z)
  - Bookmarked playbooks can optionally appear at the top
- **Visual Grouping**:
  - **Time-based grouping**: Today, Yesterday, This Week, Last Month, Older
  - **Assistant-based grouping**: Group cards by assistant with section headers
  - Collapsible group sections with item counts
- **Search**: Filter playbooks by name or goal
- **Pagination**: Support for large playbook collections
- **Actions**: Select (start agent), Delete, Bookmark toggle

### 2. Auto-Execution Flow

- **Entry Point**: User clicks "Start" button on playbook card
- **Navigation**: Routes to `/agent?playbookId={id}` → Creates session → `/agent/{sessionId}?playbookId={id}`
- **Auto-Injection**: AgentChatView detects `playbookId` query param
- **Tool Execution**: Automatically calls `selectPlaybook` tool with playbook ID
- **Workflow Trigger**: Injects tool result and triggers agent workflow
- **Agent Continuation**: Agent receives playbook details and executes steps autonomously

### 3. User Experience

- **Zero-Click Execution**: No manual command typing required
- **Visual Feedback**: Loading states during playbook selection
- **Error Handling**: Clear error messages for invalid playbooks
- **Seamless Integration**: Follows existing agent workflow patterns

---

## Architecture

### Frontend Components

#### New Features

```
src/features/playbook/
├── List.tsx              # Main playbook list page with grouping
├── Card.tsx              # Individual playbook card component
├── PlaybookGroup.tsx     # Collapsible group section component
├── SortControls.tsx      # Sort and group toggle controls
├── PlaybookViewer.tsx    # Playbook detail viewer (optional)
└── README.md             # Feature documentation
```

#### Integration Points

- **AppSidebar**: Add navigation link to `/playbooks`
- **App.tsx**: Add route for playbook list page
- **AgentChatView**: Add auto-execution logic for `playbookId` query param

### Backend Infrastructure (Existing)

- ✅ **Playbook CRUD**: `src-tauri/src/mcp/builtin/playbook/mod.rs`
- ✅ **Database Schema**: `src-tauri/src/entity/playbook.rs`
- ✅ **Operations**: `src-tauri/src/mcp/builtin/playbook/operations.rs`
- ✅ **Tool Definition**: `selectPlaybook`, `listPlaybooks`, `showPlaybooks`, etc.

### Data Flow

```
User Click "Start"
  → Navigate with ?playbookId=abc123
  → AgentChatView detects query param
  → Call backend selectPlaybook tool
  → Create tool message pair with result
  → injectMessages([toolCall, toolResult], true)
  → Backend triggers workflow
  → Agent receives playbook details & instructions
  → Agent autonomously executes workflow steps
```

---

## Implementation Details

### 1. Playbook List Component

**File**: `src/features/playbook/List.tsx`

**Pattern**: Follow `src/features/assistant/List.tsx` structure

- Grid layout with cards
- Search functionality
- Pagination support
- Context provider for playbook state

**API Integration**:

```typescript
import { listPlaybooks, deletePlaybook } from '@/lib/backend/playbooks';
```

### 2. Auto-Execution in AgentChatView

**File**: `src/features/agent/AgentChatView.tsx`

**Implementation**:

```typescript
import { useSearchParams } from 'react-router-dom';
import { createToolMessagePair } from '@/lib/chat-utils';
import { createId } from '@paralleldrive/cuid2';

function AgentChatInner() {
  const [searchParams, setSearchParams] = useSearchParams();
  const { session } = useAgentSessionState();
  const { injectMessages } = useAgentChatActions();

  useEffect(() => {
    const playbookId = searchParams.get('playbookId');
    if (playbookId && session && !isExecuting) {
      executePlaybookSelection(playbookId);
      setSearchParams({}); // Clear to prevent re-execution
    }
  }, [session, searchParams]);

  const executePlaybookSelection = async (playbookId: string) => {
    setIsExecuting(true);
    try {
      // Call backend tool
      const result = await invoke('mcp_call_tool', {
        sessionId: session.id,
        toolName: 'builtin_playbook__selectPlaybook',
        args: { id: playbookId },
      });

      // Create tool message pair
      const toolCallId = createId();
      const [toolCallMsg, toolResultMsg] = createToolMessagePair(
        'builtin_playbook__selectPlaybook',
        { id: playbookId },
        result.content,
        toolCallId,
        session.id,
        undefined,
        session.assistant?.id,
        'ui',
      );

      // Inject and trigger workflow
      await injectMessages([toolCallMsg, toolResultMsg], true);

      logger.info('Playbook auto-selected', { playbookId });
    } catch (error) {
      logger.error('Failed to auto-select playbook', error);
      toast.error('Failed to load playbook');
    } finally {
      setIsExecuting(false);
    }
  };
}
```

### 3. Bookmarking Feature

**Database**: Extend `playbooks` table with `is_bookmarked` boolean field
**Backend**: Update CRUD operations to support bookmark toggle
**Frontend**: Add bookmark button in card UI

### 4. Navigation Integration

**AppSidebar** (`src/components/layout/AppSidebar.tsx`):

```typescript
import { BookOpen } from 'lucide-react';

<SidebarMenuItem>
  <Link to="/playbooks">
    <SidebarMenuButton
      isActive={location.pathname === '/playbooks'}
      tooltip="Playbooks"
    >
      <BookOpen size={16} />
      <span>Playbooks</span>
    </SidebarMenuButton>
  </Link>
</SidebarMenuItem>
```

---

## Technical Considerations

### Tool Result Format

The `selectPlaybook` tool returns:

```rust
format!(
    "[select_playbook] Playbook \"{}\" (ID: {}) has been selected for execution.\n\nPlaybook Details:\n---\n{}\n---\n\nInstructions:\n1. Review the workflow steps and success criteria above\n2. Establish todos based on the workflow steps\n3. Begin executing the tasks according to the defined steps\n4. Track progress and verify against success criteria\n\nYou may now proceed with execution.",
    playbook.goal, playbook.id, details
);
```

This provides the agent with:

- ✅ Complete playbook context
- ✅ Step-by-step workflow
- ✅ Success criteria
- ✅ Explicit execution instructions

### Error Handling

- **Invalid Playbook ID**: Show toast error, don't start workflow
- **Database Error**: Log error, show user-friendly message
- **Session Not Ready**: Wait for session initialization before execution
- **Query Param Persistence**: Clear immediately after execution to prevent re-runs

### Performance Optimization

- **Pagination**: Load playbooks in batches (10-20 per page)
- **Lazy Loading**: Use React lazy() for playbook list component
- **Memoization**: Cache playbook list in context to avoid re-fetches

---

## Dependencies & APIs

### Existing Backend APIs

- ✅ `listPlaybooks()` - Get all playbooks for current assistant
- ✅ `getPlaybook(id)` - Get single playbook details
- ✅ `deletePlaybook(id)` - Delete playbook
- ✅ `createPlaybook(playbook)` - Create new playbook
- ✅ `updatePlaybook(playbook)` - Update existing playbook

### Existing Utilities

- ✅ `createToolMessagePair()` - Create tool call/result message pair
- ✅ `injectMessages()` - Inject messages and trigger workflow
- ✅ `invoke('mcp_call_tool')` - Execute MCP tool from frontend

### New APIs Required

- ⚠️ `togglePlaybookBookmark(id, bookmarked)` - Toggle bookmark status
- ⚠️ Database migration for `is_bookmarked` field

---

## Success Criteria

1. ✅ Users can browse all playbooks in a grid layout
2. ✅ Bookmarked playbooks appear at the top
3. ✅ Clicking "Start" creates agent session and auto-loads playbook
4. ✅ Agent automatically begins executing playbook workflow
5. ✅ No manual command typing required
6. ✅ Error states are handled gracefully
7. ✅ Page refresh doesn't re-trigger auto-execution
8. ✅ Integration follows existing architecture patterns

---

## Related Code References

### Components to Modify

- `src/components/layout/AppSidebar.tsx` - Add playbook navigation link
- `src/features/agent/AgentChatView.tsx` - Add auto-execution logic
- `src/app/App.tsx` - Add `/playbooks` route

### Components to Create

- `src/features/playbook/List.tsx` - Main playbook list page
- `src/features/playbook/Card.tsx` - Playbook card component
- `src/features/playbook/README.md` - Feature documentation

### Backend References

- `src-tauri/src/mcp/builtin/playbook/operations.rs:425-448` - selectPlaybook implementation
- `src-tauri/src/entity/playbook.rs` - Database schema
- `src/lib/backend/playbooks.ts` - Frontend API wrappers

### Utility References

- `src/lib/chat-utils.ts` - Message creation utilities
- `src/context/AgentChatContext.tsx` - Message injection
- `src/context/AgentSessionContext.tsx` - Session state management
