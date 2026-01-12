# Chat V1 Removal Refactoring Plan

## Executive Summary

**Goal:** Remove Chat V1 (legacy) session management system and migrate all remaining functionality to Agent V2 architecture.

**Impact:** High - Affects core application architecture, data layer, UI components, and backend commands.

**Timeline Estimate:** 3-4 days (Medium-sized refactoring)

**Risk Level:** Medium - Requires careful migration of History view and Settings integration

---

## 📊 Current Architecture Analysis

### Chat V1 Components (To Be Removed)

```
Frontend:
├── Contexts
│   ├── SessionContext.tsx (572 lines) - REMOVE
│   ├── SessionHistoryContext.tsx (304 lines) - REMOVE
│   └── ResourceAttachmentContext.tsx (708 lines) - REMOVE
├── Features
│   ├── features/session/
│   │   ├── SessionItem.tsx (166 lines) - REFACTOR to Agent V2
│   │   └── SessionList.tsx (86 lines) - REFACTOR to Agent V2
│   └── features/history/
│       └── History.tsx (300 lines) - REFACTOR to Agent V2
├── Services
│   └── lib/services/session-service.ts (96 lines) - REMOVE
├── Database
│   └── lib/db/ (IndexedDB)
│       ├── database.ts - REMOVE sessions table
│       ├── crud.ts - REMOVE sessionsCRUD
│       └── service.ts - REMOVE session utilities
└── Hooks
    └── hooks/use-session-navigation.ts - REFACTOR or REMOVE

Backend (Rust):
├── Commands
│   └── src-tauri/src/commands/session_commands.rs
│       ├── get_current_session_legacy() - REMOVE
│       └── list_sessions_legacy() - REMOVE
└── Session Manager
    └── src-tauri/src/session.rs - KEEP (still used for workspace management)
```

### Agent V2 Components (Keep & Enhance)

```
Frontend:
├── Contexts
│   ├── AgentSessionContext.tsx - KEEP
│   ├── AgentSessionListContext.tsx - KEEP
│   └── AgentResourceAttachmentContext.tsx - KEEP
├── Features
│   └── features/agent/ - KEEP ALL
└── Routes
    ├── /agent - KEEP
    └── /agent/:sessionId - KEEP

Backend:
├── agent/session_manager.rs - KEEP
├── agent/state.rs - KEEP
├── agent/lifecycle.rs - KEEP
├── repositories/session_repository.rs - KEEP
└── commands/agent_commands.rs - KEEP
```

---

## 🎯 Refactoring Strategy

### Phase 1: Preparation & Analysis

**Duration:** 0.5 days

#### 1.1 Audit Chat V1 Dependencies

```bash
# Find all imports
grep -r "useSessionContext\|SessionContext\|useResourceAttachment" src/
grep -r "SessionHistoryProvider\|useSessionHistory" src/
grep -r "LocalSessionService\|ISessionService" src/
```

**Affected Components:**

- ✅ `History.tsx` - Uses `useSessionContext()` for session list
- ✅ `SessionItem.tsx` - Uses `useSessionContext()` for current session
- ✅ `SettingsPage.tsx` - Uses `useSessionContext()` for factory reset
- ✅ `SessionFilesPopover.tsx` - Uses `useResourceAttachment()` (WRONG CONTEXT)
- ✅ `use-session-navigation.ts` - Uses `useSessionContext()` for navigation

#### 1.2 Data Migration Strategy

**IndexedDB → SQLite Backend**

- Chat V1 sessions stored in browser IndexedDB (ephemeral)
- Agent V2 sessions stored in Rust SQLite (persistent)
- **Decision:** No migration needed - users will start fresh with Agent V2

---

### Phase 2: Context Removal

**Duration:** 1 day

#### 2.1 Remove Chat V1 Contexts

**File:** `src/app/App.tsx`

```typescript
// REMOVE these providers:
<SessionContextProvider>        // Line ~50
  <SessionHistoryProvider>       // Line ~59
    <ResourceAttachmentProvider> // Line ~60
    // ...
    </ResourceAttachmentProvider>
  </SessionHistoryProvider>
</SessionContextProvider>
```

**File:** `src/context/SessionContext.tsx`

- **Action:** DELETE entire file (572 lines)

**File:** `src/context/SessionHistoryContext.tsx`

- **Action:** DELETE entire file (304 lines)

**File:** `src/context/ResourceAttachmentContext.tsx`

- **Action:** DELETE entire file (708 lines)

#### 2.2 Remove Service Layer

**File:** `src/lib/services/session-service.ts`

- **Action:** DELETE entire file (96 lines)

#### 2.3 Remove IndexedDB Session Tables

**File:** `src/lib/db/database.ts`

```typescript
// REMOVE sessions table definition
sessions!: Table<Session, string>; // Line 45

// REMOVE from schema
sessions: '&id, createdAt, updatedAt', // Line 64
```

**File:** `src/lib/db/crud.ts`

```typescript
// REMOVE sessionsCRUD export
export const sessionsCRUD: CRUD<Session> = { ... }; // Lines 290-320
```

**File:** `src/lib/db/service.ts`

```typescript
// REMOVE session utility methods:
-getAllSessions() - clearAllSessions() - clearSessionAndWorkspace();
```

---

### Phase 3: Component Migration

**Duration:** 1.5 days

#### 3.1 History View → Agent Session History

**File:** `src/features/history/History.tsx`

**Current Dependencies:**

```typescript
import { useSessionContext } from '@/context/SessionContext';
const { sessions, current, loadMore } = useSessionContext();
```

**Refactor To:**

```typescript
import { useAgentSessionList } from '@/context/AgentSessionListContext';

export default function History() {
  const { sessions, isSessionsListLoading } = useAgentSessionList();

  // Convert to Agent V2 session format
  const agentSessions = sessions; // Already AgentSession[]

  // Remove SessionContext dependency completely
  // Use Agent V2 navigation: /agent/:sessionId
}
```

**Changes Required:**

1. Replace `useSessionContext()` with `useAgentSessionList()`
2. Update session navigation to `/agent/:sessionId`
3. Update search integration (already uses Rust backend)
4. Remove IndexedDB session fetching
5. Use Agent V2 session metadata display

**File Changes:**

- Import: `useSessionContext` → `useAgentSessionList`
- Navigation: Update `selectAndNavigate()` to route to `/agent/:sessionId`
- Session Type: `Session` → `AgentSession`

#### 3.2 Session Components Migration

**File:** `src/features/session/SessionItem.tsx`

**Current Dependencies:**

```typescript
import { useSessionContext } from '@/context/SessionContext';
const { current, delete: onDelete } = useSessionContext();
```

**Refactor To:**

```typescript
import { useAgentSessionList } from '@/context/AgentSessionListContext';
import { useParams } from 'react-router-dom';

export default function SessionItem({ session }: { session: AgentSession }) {
  const { deleteSession } = useAgentSessionList();
  const { sessionId } = useParams();
  const isSelected = sessionId === session.id;

  const handleDelete = async () => {
    await deleteSession(session.id);
  };
}
```

**Changes Required:**

1. Accept `AgentSession` type instead of `Session`
2. Use `useParams()` for current session detection
3. Use `deleteSession()` from AgentSessionListContext
4. Update UI to show Agent V2 metadata (status, assistant)

**File:** `src/features/session/SessionList.tsx`

- Update props to accept `AgentSession[]`
- Remove Chat V1 specific fields (type, description)
- Add Agent V2 fields (status, assistant)

#### 3.3 SessionFilesPopover Fix

**File:** `src/components/shared/SessionFilesPopover.tsx`

**Current (WRONG):**

```typescript
import { useResourceAttachment } from '@/context/ResourceAttachmentContext';
const { sessionFiles } = useResourceAttachment();
```

**Option 1: Make Context-Aware (Recommended)**

```typescript
interface SessionFilesPopoverProps {
  sessionId: string;
  contextType?: 'agent' | 'chat'; // Default: 'agent'
}

export function SessionFilesPopover({
  sessionId,
  contextType = 'agent',
}: SessionFilesPopoverProps) {
  // Use appropriate context based on contextType
  const agentAttachment = useAgentResourceAttachment();

  const { sessionFiles } =
    contextType === 'agent' ? agentAttachment : { sessionFiles: [] }; // Chat V1 removed
}
```

**Option 2: Agent V2 Only (Simpler)**

```typescript
// Remove Chat V1 support completely
import { useAgentResourceAttachment } from '@/features/agent/hooks/useAgentResourceAttachment';

export function SessionFilesPopover({ sessionId }: SessionFilesPopoverProps) {
  const { sessionFiles } = useAgentResourceAttachment();
  // Filter by sessionId if needed
  const currentSessionFiles = sessionFiles.filter(
    (f) => f.sessionId === sessionId,
  );
}
```

**Decision:** Use Option 2 (Agent V2 only) - simpler and aligns with removal strategy.

#### 3.4 Settings Page Integration

**File:** `src/features/settings/SettingsPage.tsx` (Line 46)

**Current:**

```typescript
import { useSessionContext } from '@/context/SessionContext';
const sessionCtx = useSessionContext();

// Used for factory reset
await sessionCtx.factoryReset();
```

**Refactor To:**

```typescript
import { useAgentSessionList } from '@/context/AgentSessionListContext';
import { factoryReset } from '@/lib/backend/sessions'; // Rust backend

const { sessions } = useAgentSessionList();

// Factory reset
const handleFactoryReset = async () => {
  // 1. Clear Agent V2 sessions
  for (const session of sessions) {
    await deleteSession(session.id);
  }

  // 2. Call backend factory reset
  await factoryReset();

  // 3. Clear browser data
  await LocalDatabase.getInstance().playbooks.clear();
};
```

---

### Phase 4: Backend Cleanup

**Duration:** 0.5 days

#### 4.1 Remove Legacy Rust Commands

**File:** `src-tauri/src/commands/session_commands.rs`

**Remove:**

```rust
// Line 364
#[command]
pub async fn get_current_session_legacy() -> Result<Option<String>, String> { ... }

// Line 377
#[command]
pub async fn list_sessions_legacy() -> Result<Vec<String>, String> { ... }
```

**File:** `src-tauri/src/lib.rs`

**Remove from command list:**

```rust
// Lines 47-48, 255, 257
get_current_session_legacy,
list_sessions_legacy,
```

#### 4.2 Update Session Type Model

**File:** `src/models/chat.ts`

**Remove Chat V1 Session interface:**

```typescript
// Lines 210-260 (approximate)
export interface Session {
  id: string;
  name?: string;
  type: 'single' | 'multi';
  assistants: Assistant[];
  description?: string;
  sessionThread: Thread;
  createdAt: Date;
  updatedAt: Date;
}
```

**Keep only:** `AgentSession` (already defined in `src/models/agent.ts`)

---

### Phase 5: Route & Navigation Updates

**Duration:** 0.5 days

#### 5.1 Update Navigation Hook

**File:** `src/hooks/use-session-navigation.ts`

**Current:**

```typescript
import { useSessionContext } from '@/context/SessionContext';

export function useSessionNavigation() {
  const { select } = useSessionContext();

  const selectAndNavigate = (sessionId: string) => {
    select(sessionId);
    navigate(`/history`); // Chat V1 route
  };
}
```

**Refactor To:**

```typescript
import { useNavigate } from 'react-router-dom';

export function useAgentNavigation() {
  const navigate = useNavigate();

  const navigateToSession = (sessionId: string) => {
    navigate(`/agent/${sessionId}`); // Agent V2 route
  };

  const navigateToAgentHome = () => {
    navigate('/agent');
  };
}
```

#### 5.2 Sidebar Integration

**File:** `src/components/layout/AppSidebar.tsx`

**Update session list display:**

- Use `useAgentSessionList()` instead of `useSessionContext()`
- Render `AgentSession[]` with status indicators
- Navigate to `/agent/:sessionId` on click

---

## 📋 Detailed Migration Checklist

### Frontend Files to Delete

- [ ] `src/context/SessionContext.tsx` (572 lines)
- [ ] `src/context/SessionHistoryContext.tsx` (304 lines)
- [ ] `src/context/ResourceAttachmentContext.tsx` (708 lines)
- [ ] `src/lib/services/session-service.ts` (96 lines)
- [ ] `src/models/ui/legacy-attachment.ts` (if unused after ResourceAttachmentContext removal)

### Frontend Files to Refactor

- [ ] `src/app/App.tsx` - Remove Chat V1 providers
- [ ] `src/features/history/History.tsx` - Use Agent V2 context
- [ ] `src/features/session/SessionItem.tsx` - Accept AgentSession type
- [ ] `src/features/session/SessionList.tsx` - Accept AgentSession[] type
- [ ] `src/components/shared/SessionFilesPopover.tsx` - Use Agent V2 context
- [ ] `src/features/settings/SettingsPage.tsx` - Update factory reset
- [ ] `src/hooks/use-session-navigation.ts` - Rename to useAgentNavigation
- [ ] `src/components/layout/AppSidebar.tsx` - Use Agent V2 session list

### Database Files to Update

- [ ] `src/lib/db/database.ts` - Remove sessions table
- [ ] `src/lib/db/crud.ts` - Remove sessionsCRUD
- [ ] `src/lib/db/service.ts` - Remove session utilities

### Backend Files to Update

- [ ] `src-tauri/src/commands/session_commands.rs` - Remove legacy commands
- [ ] `src-tauri/src/lib.rs` - Remove command registrations

### Type Definitions to Update

- [ ] `src/models/chat.ts` - Remove Session interface
- [ ] `src/models/search.ts` - Update SessionWithHits to use AgentSession

---

## 🚨 Breaking Changes & Migration Guide

### For Users

**Impact:** Existing Chat V1 sessions will NOT be migrated

- **Action:** Users must recreate sessions in Agent V2
- **Rationale:** Different architecture, no automated migration path
- **Mitigation:** Add migration notice in release notes

### For Developers

**API Changes:**

```typescript
// BEFORE (Chat V1)
import { useSessionContext } from '@/context/SessionContext';
const { sessions, current } = useSessionContext();

// AFTER (Agent V2)
import { useAgentSessionList } from '@/context/AgentSessionListContext';
const { sessions } = useAgentSessionList();
```

**Navigation Changes:**

```typescript
// BEFORE
navigate('/history'); // View session list
select(sessionId); // Activate session

// AFTER
navigate('/agent'); // View session list
navigate(`/agent/${sessionId}`); // Open session
```

**Session Type Changes:**

```typescript
// BEFORE
interface Session {
  type: 'single' | 'multi';
  assistants: Assistant[];
  sessionThread: Thread;
}

// AFTER
interface AgentSession {
  status: 'idle' | 'busy' | 'paused' | 'error';
  assistant?: Assistant; // Single assistant only
}
```

---

## 🧪 Testing Strategy

### Unit Tests

- [ ] Test `History.tsx` with Agent V2 context
- [ ] Test `SessionFilesPopover` with Agent V2 context
- [ ] Test factory reset without Chat V1 dependencies
- [ ] Test navigation to `/agent/:sessionId`

### Integration Tests

- [ ] Test session list loading from Agent V2 backend
- [ ] Test session deletion workflow
- [ ] Test file attachment display in Agent V2
- [ ] Test search integration with Agent V2 sessions

### Manual Testing

- [ ] Verify `/agent` route shows session list
- [ ] Verify `/agent/:sessionId` opens session correctly
- [ ] Verify `/history` shows Agent V2 sessions with search
- [ ] Verify settings factory reset works
- [ ] Verify file attachments display in SessionFilesPopover
- [ ] Verify no console errors related to removed contexts

---

## 📦 Rollout Plan

### Step 1: Feature Flag (Optional)

```typescript
const USE_AGENT_V2_ONLY = true; // Feature flag

// In App.tsx
{!USE_AGENT_V2_ONLY && (
  <SessionContextProvider>
    {/* Chat V1 components */}
  </SessionContextProvider>
)}
```

### Step 2: Gradual Removal

1. **Week 1:** Remove contexts, migrate History view
2. **Week 2:** Migrate SessionFilesPopover, update Settings
3. **Week 3:** Remove backend commands, cleanup database
4. **Week 4:** QA testing, release

### Step 3: Documentation

- [ ] Update README.md with Agent V2 architecture
- [ ] Update CONTRIBUTING.md with new context patterns
- [ ] Add migration guide for developers
- [ ] Update API documentation

---

## 🎯 Success Criteria

- ✅ No references to `SessionContext`, `SessionHistoryContext`, `ResourceAttachmentContext`
- ✅ All UI components use Agent V2 contexts
- ✅ IndexedDB sessions table removed
- ✅ Legacy Rust commands removed
- ✅ History view functional with Agent V2 backend
- ✅ SessionFilesPopover shows correct file count
- ✅ Settings factory reset works with Agent V2
- ✅ No console errors or warnings
- ✅ All tests passing

---

## 🔧 Implementation Order

### Recommended Sequence (Minimize Breaking Changes)

1. **Day 1 Morning:** Phase 1 - Audit & Documentation
2. **Day 1 Afternoon:** Phase 3.3 - Fix SessionFilesPopover (Quick Win)
3. **Day 2 Morning:** Phase 3.1 - Migrate History.tsx
4. **Day 2 Afternoon:** Phase 3.2 - Migrate SessionItem/SessionList
5. **Day 3 Morning:** Phase 3.4 - Update SettingsPage
6. **Day 3 Afternoon:** Phase 2 - Remove Contexts & Providers
7. **Day 4 Morning:** Phase 4 - Backend Cleanup
8. **Day 4 Afternoon:** Phase 5 - Testing & QA

---

## 📝 Notes

### Why Remove Instead of Maintain?

1. **Dual Context Complexity:** Two parallel session systems cause bugs (SessionFilesPopover)
2. **Technical Debt:** Chat V1 uses outdated patterns (IndexedDB vs SQLite)
3. **Maintenance Burden:** Supporting both architectures doubles complexity
4. **User Confusion:** Two different session management UIs
5. **Agent V2 Superior:** Better architecture, persistent storage, event-driven updates

### Risks & Mitigations

| Risk                         | Impact | Mitigation                            |
| ---------------------------- | ------ | ------------------------------------- |
| Users lose existing sessions | High   | Add migration notice, document export |
| Regression in History view   | Medium | Thorough testing, feature flag        |
| SessionFilesPopover breaks   | High   | Fix immediately (Day 1)               |
| Factory reset fails          | Medium | Test multiple scenarios               |
| TypeScript errors            | Low    | Incremental removal, type checks      |

---

## 🚀 Next Steps

1. **Review this plan** with team
2. **Create GitHub issues** for each phase
3. **Set up feature branch:** `feat/remove-chat-v1`
4. **Begin Phase 1:** Audit dependencies
5. **Track progress** in project board

---

**Plan Version:** 1.0  
**Created:** 2026-01-11  
**Last Updated:** 2026-01-11  
**Status:** Draft - Awaiting Approval
