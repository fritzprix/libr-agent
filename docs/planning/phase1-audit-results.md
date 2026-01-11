# Phase 1: Chat V1 Dependency Audit Results

**Date:** 2026-01-11  
**Status:** ✅ Complete  
**Duration:** Completed in Phase 1

---

## Executive Summary

**Total Files Affected:** 15 files directly depend on Chat V1 contexts  
**Total Lines of Chat V1 Code:** ~1,686 lines (3 context files + 1 service file)  
**Critical Dependencies:** 3 core components require immediate attention

---

## 🔍 Detailed Dependency Analysis

### 1. SessionContext Dependencies

**Context File:** `src/context/SessionContext.tsx` (572 lines)

#### Direct Consumers (7 files)

1. ✅ **`src/app/App.tsx`** (Lines 19, 46, 119)
   - Provider wrapper at app root
   - **Action:** Remove provider from hierarchy
   - **Risk:** Low - isolated change

2. 🔴 **`src/hooks/use-session-navigation.ts`** (Lines 3, 20)
   - Uses: `useSessionContext()` for `current`, `select()`
   - **Action:** Refactor to Agent V2 navigation
   - **Risk:** Medium - affects navigation across app

3. 🔴 **`src/features/history/History.tsx`** (Lines 11, 28)
   - Uses: `useSessionContext()` for session list, pagination
   - **Action:** Migrate to `useAgentSessionList()`
   - **Risk:** High - main user-facing feature

4. 🔴 **`src/features/session/SessionItem.tsx`** (Lines 3, 36)
   - Uses: `useSessionContext()` for `current`, `delete()`
   - **Action:** Use `useParams()` + `useAgentSessionList()`
   - **Risk:** Medium - shared component

5. 🔴 **`src/features/settings/SettingsPage.tsx`** (Lines 46, 163)
   - Uses: `useSessionContext()` for `factoryReset()`
   - **Action:** Call backend directly
   - **Risk:** Low - isolated usage

6. ⚠️ **`src/features/tools/index.tsx`** (Lines 7, 70)
   - Uses: `useSessionContext()` for `getCurrentSession()`, `current`
   - **Action:** Investigate if needed for BuiltInToolProvider
   - **Risk:** Medium - core tool system

7. ✅ **`src/context/SessionHistoryContext.tsx`** (Lines 12, 72)
   - Uses: `useSessionContext()` for `current`
   - **Action:** Will be deleted with SessionHistoryContext
   - **Risk:** None - part of removal

8. ✅ **`src/context/ResourceAttachmentContext.tsx`** (Lines 15, 86)
   - Uses: `useSessionContext()` for `current`, `updateSession()`
   - **Action:** Will be deleted with ResourceAttachmentContext
   - **Risk:** None - part of removal

#### Indirect References (Agent V2 - KEEP)

- `src/context/AgentSessionContext.tsx` - Agent V2 (different)
- `src/context/AgentChatContext.tsx` - Agent V2 (different)
- All `src/features/agent/**` files - Agent V2 (different)

---

### 2. ResourceAttachmentContext Dependencies

**Context File:** `src/context/ResourceAttachmentContext.tsx` (708 lines)

#### Direct Consumers (2 files)

1. ✅ **`src/app/App.tsx`** (Line 24)
   - Provider wrapper at app root
   - **Action:** Remove provider from hierarchy
   - **Risk:** Low - isolated change

2. 🔴 **`src/components/shared/SessionFilesPopover.tsx`** (Lines 12, 25)
   - Uses: `useResourceAttachment()` for `sessionFiles`
   - **Status:** ❌ WRONG CONTEXT (bug)
   - **Action:** Switch to `useAgentResourceAttachment()`
   - **Risk:** High - currently broken in Agent V2
   - **Priority:** 🚨 IMMEDIATE FIX REQUIRED

#### Related Types

- `src/models/ui/legacy-attachment.ts` - Legacy types
  - **Action:** Review if used elsewhere, likely can be deleted

---

### 3. SessionHistoryContext Dependencies

**Context File:** `src/context/SessionHistoryContext.tsx` (304 lines)

#### Direct Consumers (1 file)

1. ✅ **`src/app/App.tsx`** (Lines 20, 59, 116)
   - Provider wrapper at app root
   - **Action:** Remove provider from hierarchy
   - **Risk:** Low - no active consumers found
   - **Note:** SessionHistoryContext NOT used in History.tsx (uses backend directly)

#### Usage Analysis

- ⚠️ **NO ACTIVE CONSUMERS FOUND** in grep search
- Context exists but appears unused in current codebase
- **Action:** Safe to delete immediately

---

### 4. LocalSessionService Dependencies

**Service File:** `src/lib/services/session-service.ts` (96 lines)

#### Direct Consumers (1 file)

1. ✅ **`src/context/SessionContext.tsx`** (Lines 17, 122)
   - Used by: `SessionContextProvider`
   - **Action:** Will be deleted with SessionContext
   - **Risk:** None - part of removal

#### Database Integration

- Uses `dbService.sessions` from IndexedDB
- Uses `dbUtils` for session management
- **Action:** Remove after SessionContext removal

---

### 5. IndexedDB Session Table

**Database Files:**

- `src/lib/db/database.ts` - Session table definition
- `src/lib/db/crud.ts` - sessionsCRUD operations
- `src/lib/db/service.ts` - Session utility methods

#### Session Table Schema

```typescript
sessions!: Table<Session, string>;
// Schema: '&id, createdAt, updatedAt'
```

#### Operations to Remove

- `sessionsCRUD.upsert()`
- `sessionsCRUD.read()`
- `sessionsCRUD.delete()`
- `sessionsCRUD.getPage()`
- `dbUtils.getAllSessions()`
- `dbUtils.clearAllSessions()`
- `dbUtils.clearSessionAndWorkspace()`

---

## 📊 Dependency Matrix

| Component                 | SessionContext | ResourceAttachment | SessionHistory | Priority |
| ------------------------- | -------------- | ------------------ | -------------- | -------- |
| App.tsx                   | ✅ Provider    | ✅ Provider        | ✅ Provider    | P1       |
| History.tsx               | 🔴 Consumer    | -                  | -              | P1       |
| SessionFilesPopover       | -              | 🔴 Wrong Context   | -              | P0       |
| SessionItem.tsx           | 🔴 Consumer    | -                  | -              | P2       |
| SettingsPage.tsx          | 🔴 Consumer    | -                  | -              | P3       |
| use-session-navigation.ts | 🔴 Consumer    | -                  | -              | P2       |
| tools/index.tsx           | ⚠️ Consumer    | -                  | -              | P2       |

**Legend:**

- P0: Critical bug fix (immediate)
- P1: Core functionality (day 1-2)
- P2: Secondary features (day 2-3)
- P3: Low impact (day 3-4)

---

## 🎯 Critical Findings

### Finding 1: SessionFilesPopover Bug 🚨

**File:** `src/components/shared/SessionFilesPopover.tsx`

**Issue:**

```typescript
// ❌ WRONG: Using Chat V1 context in Agent V2 views
import { useResourceAttachment } from '@/context/ResourceAttachmentContext';
const { sessionFiles } = useResourceAttachment();
```

**Impact:**

- Shows "0 files" even after successful upload in Agent V2
- Used in `AgentChatHeader.tsx` (Agent V2 view)
- Context mismatch between Chat V1 and Agent V2

**Solution:**

```typescript
// ✅ CORRECT: Use Agent V2 context
import { useAgentResourceAttachment } from '@/features/agent/hooks/useAgentResourceAttachment';
const { sessionFiles } = useAgentResourceAttachment();
```

**Priority:** P0 - Fix immediately before other changes

---

### Finding 2: History.tsx Migration Complexity

**File:** `src/features/history/History.tsx`

**Current State:**

- Uses `useSessionContext()` for session pagination
- Uses SWR for search (already points to Rust backend)
- Complex state management with `SessionWithHits` type

**Migration Path:**

1. Replace `useSessionContext()` with `useAgentSessionList()`
2. Update `SessionWithHits` to extend `AgentSession`
3. Update navigation to `/agent/:sessionId`
4. Remove Chat V1 session type handling

**Estimated Effort:** 3-4 hours

---

### Finding 3: BuiltInToolProvider Session Dependency

**File:** `src/features/tools/index.tsx`

**Usage:**

```typescript
const { getCurrentSession, current: currentSession } = useSessionContext();
```

**Purpose Analysis:**

- Used for session context switching in tool execution
- May need to remain for legacy MCP tool compatibility
- **Action Required:** Further investigation needed

**Options:**

1. Remove if only used for Agent V2 (likely)
2. Keep and adapt to use Agent V2 session
3. Make context-aware (dual support)

**Estimated Investigation Time:** 1-2 hours

---

## 🗂️ Files to Delete (Confirmed)

### Phase 2: Context Removal

1. ✅ `src/context/SessionContext.tsx` (572 lines)
2. ✅ `src/context/SessionHistoryContext.tsx` (304 lines)
3. ✅ `src/context/ResourceAttachmentContext.tsx` (708 lines)
4. ✅ `src/lib/services/session-service.ts` (96 lines)
5. ✅ `src/models/ui/legacy-attachment.ts` (if unused)

**Total Lines to Delete:** ~1,686 lines

### Database Cleanup (Partial Deletion)

- `src/lib/db/database.ts` - Remove sessions table (10-15 lines)
- `src/lib/db/crud.ts` - Remove sessionsCRUD (30-40 lines)
- `src/lib/db/service.ts` - Remove session utilities (50-60 lines)

**Total Lines to Delete:** ~90-115 lines

---

## 🔧 Files to Refactor

### High Priority (P1)

1. 🔴 `src/components/shared/SessionFilesPopover.tsx`
   - **Lines:** ~247 lines total
   - **Changes:** Import + 1-2 lines
   - **Effort:** 15 minutes
   - **Risk:** Low

2. 🔴 `src/features/history/History.tsx`
   - **Lines:** ~300 lines total
   - **Changes:** Context replacement, type updates
   - **Effort:** 3-4 hours
   - **Risk:** Medium

### Medium Priority (P2)

3. 🔴 `src/features/session/SessionItem.tsx`
   - **Lines:** ~166 lines total
   - **Changes:** Context replacement, prop types
   - **Effort:** 2-3 hours
   - **Risk:** Medium

4. 🔴 `src/features/session/SessionList.tsx`
   - **Lines:** ~86 lines total
   - **Changes:** Prop type updates
   - **Effort:** 1 hour
   - **Risk:** Low

5. 🔴 `src/hooks/use-session-navigation.ts`
   - **Lines:** ~50 lines total (estimated)
   - **Changes:** Complete rewrite for Agent V2
   - **Effort:** 1-2 hours
   - **Risk:** Medium

6. ⚠️ `src/features/tools/index.tsx`
   - **Lines:** ~461 lines total
   - **Changes:** Remove or adapt session context usage
   - **Effort:** 2-3 hours (investigation + implementation)
   - **Risk:** High - core system

### Low Priority (P3)

7. 🔴 `src/features/settings/SettingsPage.tsx`
   - **Lines:** ~977 lines total
   - **Changes:** Replace factoryReset call
   - **Effort:** 30 minutes
   - **Risk:** Low

8. ✅ `src/app/App.tsx`
   - **Lines:** ~150 lines total (provider section)
   - **Changes:** Remove 3 provider wrappers
   - **Effort:** 10 minutes
   - **Risk:** Low

---

## 📝 Type Definitions to Update

### 1. Session Interface (src/models/chat.ts)

**Current:**

```typescript
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

**Action:** DELETE (lines 273-282 approximately)

**Replacement:** Use `AgentSession` from `src/models/agent.ts`

### 2. SessionWithHits (src/models/search.ts)

**Current:**

```typescript
export interface SessionWithHits extends Session {
  searchHits?: number;
}
```

**Action:** Update to extend `AgentSession`

```typescript
export interface SessionWithHits extends AgentSession {
  searchHits?: number;
}
```

---

## 🧪 Test Files Affected

### Tests to Update

1. ✅ `src/context/__tests__/AgentSessionContext.test.tsx`
   - **Status:** Agent V2 (KEEP)
   - **Action:** None

2. ✅ `src/context/__tests__/AgentSessionListContext.test.tsx`
   - **Status:** Agent V2 (KEEP)
   - **Action:** None

### Tests to Delete

- No Chat V1 test files found
- **Note:** SessionContext tests likely never existed or already deleted

---

## 🚨 Risk Assessment

### High Risk Areas

#### 1. History View Migration

**Risk:** User-facing feature, complex state management  
**Mitigation:**

- Implement behind feature flag
- Thorough manual testing
- Preserve search functionality

#### 2. BuiltInToolProvider Session Usage

**Risk:** Core tool system dependency  
**Mitigation:**

- Investigation phase before changes
- Test with all tool types
- Verify MCP server communication

### Medium Risk Areas

#### 3. Navigation System Changes

**Risk:** App-wide routing changes  
**Mitigation:**

- Update all navigation calls systematically
- Test all routes
- Verify browser history behavior

#### 4. Session Type Changes

**Risk:** TypeScript compilation errors across codebase  
**Mitigation:**

- Incremental type updates
- Leverage TypeScript compiler for detection
- Fix all type errors before testing

### Low Risk Areas

#### 5. Provider Removal

**Risk:** React context tree changes  
**Mitigation:**

- Remove from App.tsx in single commit
- Test app initialization
- Verify no runtime errors

---

## 📈 Estimated Timeline

### Day 1 (4 hours)

- ✅ Phase 1 Complete (this document)
- 🔴 Fix SessionFilesPopover (15 min)
- 🔴 Investigate BuiltInToolProvider usage (2 hours)
- 🔴 Begin History.tsx migration (1.5 hours)

### Day 2 (6 hours)

- 🔴 Complete History.tsx migration (2 hours)
- 🔴 Migrate SessionItem.tsx (2 hours)
- 🔴 Migrate SessionList.tsx (1 hour)
- 🔴 Update use-session-navigation.ts (1 hour)

### Day 3 (4 hours)

- 🔴 Update SettingsPage.tsx (30 min)
- ✅ Remove contexts from App.tsx (10 min)
- ✅ Delete context files (5 min)
- ✅ Update database files (1 hour)
- 🧪 Testing and validation (2 hours)

### Day 4 (2 hours)

- 🔧 Backend cleanup (Rust commands)
- 🧪 Final QA and regression testing
- 📝 Update documentation

**Total Estimated Time:** 16 hours (2 days of focused work)

---

## ✅ Phase 1 Checklist

- [x] Audit SessionContext dependencies (7 direct consumers)
- [x] Audit ResourceAttachmentContext dependencies (2 direct consumers)
- [x] Audit SessionHistoryContext dependencies (0 active consumers)
- [x] Identify files to delete (5 files, ~1,686 lines)
- [x] Identify files to refactor (8 files)
- [x] Assess risks and mitigation strategies
- [x] Create estimated timeline
- [x] Document critical findings
- [x] Identify immediate fix needed (SessionFilesPopover)

---

## 🎯 Next Steps (Phase 2)

### Immediate Action Items

1. **P0 Fix:** SessionFilesPopover context bug (15 min)
2. **Investigation:** BuiltInToolProvider session usage (2 hours)
3. **Begin Migration:** History.tsx to Agent V2 (start Day 1)

### Phase 2 Entry Criteria

- ✅ Phase 1 audit complete
- ✅ Team approval received
- ✅ Feature branch created (`feat/remove-chat-v1`)
- ✅ Backup/checkpoint created

---

## 📊 Summary Statistics

| Metric                    | Count                |
| ------------------------- | -------------------- |
| Files to Delete           | 5                    |
| Files to Refactor         | 8                    |
| Lines to Delete           | ~1,776               |
| Lines to Modify           | ~500-700 (estimated) |
| Components Affected       | 15                   |
| Test Files Affected       | 0                    |
| Rust Commands to Remove   | 2                    |
| Database Tables to Remove | 1                    |
| Estimated Effort          | 16 hours             |
| Risk Level                | Medium               |

---

**Phase 1 Status:** ✅ **COMPLETE**  
**Ready for Phase 2:** ✅ **YES**  
**Critical Issues Found:** 1 (SessionFilesPopover bug)  
**Blockers:** None

**Next Phase:** [Phase 2 - Context Removal](./phase2-context-removal.md) (to be created)
