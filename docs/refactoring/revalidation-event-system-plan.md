# Revalidation Event System Implementation Plan

## Executive Summary

**Goal**: Ensure frontend cache revalidation works for both UI-initiated and AI agent-initiated resource updates (assistants, MCP servers).

**Current Problem**:

- UI operations → ✅ Frontend revalidates
- AI agent operations → ❌ Frontend doesn't revalidate

**Solution**: Extend existing `agent:event` system with `ResourceUpdated` events.

**Implementation Time**: ~1.5 hours
**Risk Level**: Low
**Breaking Changes**: None

---

## Architecture Overview

### Current Flow (UI-Initiated)

```
User UI Action
  ↓
RustAssistantService.save()
  ↓
Tauri Command (create_assistant)
  ↓
Database Update
  ↓
RustAssistantService.emitRevalidate() ✅
  ↓
AssistantContext.loadAssistants()
  ↓
UI Updates
```

### Current Flow (AI Agent-Initiated)

```
AI Agent Tool Call
  ↓
builtin_assistant__createAssistant
  ↓
operations::create_assistant()
  ↓
Database Update
  ↓
Backend cache.invalidate_cache() ✅
  ↓
❌ NO FRONTEND EVENT
  ↓
UI STALE ❌
```

### New Flow (Unified)

```
Any Operation (UI or AI Agent)
  ↓
Rust Backend Operation
  ↓
Database Update
  ↓
Backend cache.invalidate_cache() ✅
  ↓
emit_agent_event(ResourceUpdated) ✅ NEW
  ↓
Frontend Event Listener ✅ NEW
  ↓
AssistantContext.loadAssistants()
  ↓
UI Updates ✅
```

---

## Phase 1: Backend Infrastructure (30 min)

### Step 1.1: Add Global AppHandle Storage (5 min)

**File**: `src-tauri/src/state.rs`

**Location**: After line 48 (after `PLANNING_REPOSITORY`)

```rust
use tauri::AppHandle;

/// A global, thread-safe, once-initialized Tauri AppHandle for event emission.
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Initialize the global AppHandle
/// Should be called once during application setup
pub fn init_app_handle(handle: AppHandle) {
    if APP_HANDLE.set(handle).is_err() {
        log::warn!("AppHandle already initialized");
    }
}

/// Get the global AppHandle for event emission
/// Returns None if not initialized yet
pub fn get_app_handle() -> Option<&'static AppHandle> {
    APP_HANDLE.get()
}
```

**Test Point**: Verify `get_app_handle()` returns Some after app initialization.

---

### Step 1.2: Initialize AppHandle in Tauri Setup (3 min)

**File**: `src-tauri/src/lib.rs`

**Location**: In `.setup()` hook, right after other initializations (around line 540)

**Find**:

```rust
            .setup(|app| {
                info!("Starting LibrAgent...");

                // Initialize database and run migrations
                let db_url = init_sqlite().map_err(|e| {
```

**Add After** (around line 555, after database initialization):

```rust
                // Initialize global AppHandle for event emission
                crate::state::init_app_handle(app.handle().clone());
                info!("✅ Global AppHandle initialized for event emission");
```

**Test Point**: Check logs for "✅ Global AppHandle initialized" message on startup.

---

### Step 1.3: Add ResourceUpdated Event Type (5 min)

**File**: `src-tauri/src/agent/events.rs`

**Location**: Add new variant to `AgentEvent` enum (around line 67, before closing brace)

**Find**:

```rust
    /// Session initialization step update
    #[serde(rename_all = "camelCase")]
    InitializationStep {
        session_id: String,
        step: String,
        status: InitializationStatus,
    },
}
```

**Replace With**:

```rust
    /// Session initialization step update
    #[serde(rename_all = "camelCase")]
    InitializationStep {
        session_id: String,
        step: String,
        status: InitializationStatus,
    },

    /// Resource updated (assistants, MCP servers, playbooks, etc.)
    /// Emitted when builtin tools modify global resources
    #[serde(rename_all = "camelCase")]
    ResourceUpdated {
        /// Type of resource: "assistant" | "mcpServer" | "playbook"
        resource_type: String,
        /// Action performed: "create" | "update" | "delete"
        action: String,
        /// Optional resource identifier
        resource_id: Option<String>,
    },
}
```

**Test Point**: Compile check - `cargo build` should succeed.

---

### Step 1.4: Create Event Emission Helper (5 min)

**File**: `src-tauri/src/agent/events.rs`

**Location**: Add new helper function after `emit_agent_event` (around line 80)

```rust
/// Emit a resource update event (convenience wrapper)
///
/// This is a shorthand for emitting ResourceUpdated events from builtin tools.
/// Falls back silently if AppHandle is not available (e.g., during tests).
pub fn emit_resource_updated(
    resource_type: &str,
    action: &str,
    resource_id: Option<String>,
) {
    if let Some(app_handle) = crate::state::get_app_handle() {
        let event = AgentEvent::ResourceUpdated {
            resource_type: resource_type.to_string(),
            action: action.to_string(),
            resource_id,
        };

        if let Err(e) = emit_agent_event(app_handle, event) {
            log::warn!("Failed to emit resource update event: {}", e);
        }
    } else {
        log::debug!(
            "AppHandle not available, skipping resource update event (resource_type: {}, action: {})",
            resource_type,
            action
        );
    }
}
```

**Rationale**:

- Graceful degradation if AppHandle not available (tests, early initialization)
- Centralized logging for debugging
- Clean API for builtin tools

**Test Point**: Unit test with mock AppHandle.

---

### Step 1.5: Add Event Emission to Assistant Operations (12 min)

**File**: `src-tauri/src/mcp/builtin/assistant/operations.rs`

#### 1.5.1: Add Import (Line 1)

**Find**:

```rust
use crate::mcp::builtin::error_guidance::{
    duplicate_error, invalid_input_error, not_found_error, operation_failed_error, SuccessHint,
    ToolGroup,
};
```

**Add After**:

```rust
use crate::agent::events;
```

#### 1.5.2: createAssistant Event Emission (Line 257)

**Find**:

```rust
            server.invalidate_cache().await;

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "id": id,
                "name": request.name
            }))))
```

**Replace With**:

```rust
            server.invalidate_cache().await;

            // Emit resource updated event for frontend cache revalidation
            events::emit_resource_updated("assistant", "create", Some(id.clone()));

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "id": id,
                "name": request.name
            }))))
```

#### 1.5.3: updateAssistant Event Emission (Line 360)

**Find**:

```rust
            server.invalidate_cache().await;

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "id": request.id,
                "name": name
            }))))
```

**Replace With**:

```rust
            server.invalidate_cache().await;

            // Emit resource updated event for frontend cache revalidation
            events::emit_resource_updated("assistant", "update", Some(request.id.clone()));

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "success": true,
                "id": request.id,
                "name": name
            }))))
```

#### 1.5.4: deleteAssistant Event Emission (Line 412)

**Find**:

```rust
            server.invalidate_cache().await;

            Ok(hint.to_mcp_result())
```

**Replace With**:

```rust
            server.invalidate_cache().await;

            // Emit resource updated event for frontend cache revalidation
            events::emit_resource_updated("assistant", "delete", Some(request.id.clone()));

            Ok(hint.to_mcp_result())
```

**Test Point**:

- Compile check
- Runtime test: Call `builtin_assistant__createAssistant` and verify event emission in logs

---

## Phase 2: Frontend Infrastructure (45 min)

### Step 2.1: Add TypeScript Event Types (5 min)

**File**: `src/context/AgentSessionContext.tsx`

**Location**: Add to `AgentEventPayload` type union (around line 50)

**Find**:

```typescript
  | {
      type: 'initializationStep';
      sessionId: string;
      step: string;
      status: 'running' | 'complete' | 'error';
    };
```

**Add After**:

```typescript
  | {
      type: 'resourceUpdated';
      resourceType: 'assistant' | 'mcpServer' | 'playbook';
      action: 'create' | 'update' | 'delete';
      resourceId?: string;
    };
```

**Test Point**: TypeScript compile check.

---

### Step 2.2: Add Event Listener to AssistantContext (15 min)

**File**: `src/context/AssistantContext.tsx`

**Location**: Add new useEffect after existing service subscription (around line 410)

**Find**:

```typescript
// Subscribe to local service events (Main Thread changes)
useEffect(() => {
  const unsubscribe = assistantService.onRevalidate((event) => {
    logger.debug('Local assistant service changed, refreshing...', event);
    loadAssistants();
  });
  return unsubscribe;
}, [assistantService, loadAssistants]);
```

**Add After**:

```typescript
// Subscribe to agent:event for AI agent tool updates
useEffect(() => {
  let unlisten: (() => void) | undefined;

  const setupListener = async () => {
    try {
      const { listen } = await import('@tauri-apps/api/event');

      unlisten = await listen<AgentEventPayload>('agent:event', (event) => {
        const payload = event.payload;

        // Only handle resourceUpdated events for assistants
        if (
          payload.type === 'resourceUpdated' &&
          payload.resourceType === 'assistant'
        ) {
          logger.info('Assistant resource updated via agent event', {
            action: payload.action,
            resourceId: payload.resourceId,
          });

          // Refresh assistant list (debounced by React state)
          loadAssistants();
        }
      });

      logger.debug('Agent event listener registered for assistant updates');
    } catch (error) {
      logger.error('Failed to setup agent event listener', error);
    }
  };

  setupListener();

  return () => {
    if (unlisten) {
      unlisten();
      logger.debug('Agent event listener unregistered');
    }
  };
}, [loadAssistants]);
```

**Rationale**:

- Dynamic import for @tauri-apps/api to avoid SSR issues
- Type-safe payload handling
- Proper cleanup on unmount
- Logging for debugging

**Test Point**:

- Component mounts without errors
- Event listener registered (check logs)
- Manual event emission triggers `loadAssistants()`

---

### Step 2.3: Add AgentEventPayload Import (2 min)

**File**: `src/context/AssistantContext.tsx`

**Location**: Add to imports at top of file (around line 20)

**Find**:

```typescript
import { AssistantService } from '@/lib/services/assistant-service';
```

**Add After**:

```typescript
import type { AgentEventPayload } from './AgentSessionContext';
```

---

### Step 2.4: Add Event Listener to MCPServerRegistryContext (15 min)

**File**: `src/context/MCPServerRegistryContext.tsx`

**Location**: Add new useEffect after existing service subscription (around line 170)

**Find**:

```typescript
// Subscribe to local service events (Main Thread changes)
useEffect(() => {
  const unsubscribe = mcpServerService.onRevalidate((event) => {
    logger.debug('Local service changed, refreshing...', event);
    refreshAll();
  });
  return unsubscribe;
}, [mcpServerService, refreshAll]);
```

**Add After**:

```typescript
// Subscribe to agent:event for AI agent tool updates
useEffect(() => {
  let unlisten: (() => void) | undefined;

  const setupListener = async () => {
    try {
      const { listen } = await import('@tauri-apps/api/event');

      unlisten = await listen<AgentEventPayload>('agent:event', (event) => {
        const payload = event.payload;

        // Only handle resourceUpdated events for MCP servers
        if (
          payload.type === 'resourceUpdated' &&
          payload.resourceType === 'mcpServer'
        ) {
          logger.info('MCP server resource updated via agent event', {
            action: payload.action,
            resourceId: payload.resourceId,
          });

          // Refresh server list
          refreshAll();
        }
      });

      logger.debug('Agent event listener registered for MCP server updates');
    } catch (error) {
      logger.error('Failed to setup agent event listener', error);
    }
  };

  setupListener();

  return () => {
    if (unlisten) {
      unlisten();
      logger.debug('Agent event listener unregistered');
    }
  };
}, [refreshAll]);
```

**Test Point**: Same as AssistantContext listener.

---

### Step 2.5: Add AgentEventPayload Import to MCPServerRegistryContext (2 min)

**File**: `src/context/MCPServerRegistryContext.tsx`

**Location**: Add to imports (around line 15)

**Find**:

```typescript
import { getLogger } from '@/lib/logger';
```

**Add After**:

```typescript
import type { AgentEventPayload } from './AgentSessionContext';
```

---

### Step 2.6: Add Debouncing for Rapid Updates (6 min)

**File**: `src/context/AssistantContext.tsx`

**Location**: Add debounce ref and logic

**Find** (in component body, around line 160):

```typescript
const currentAssistantRef = useRef(currentAssistant);
```

**Add After**:

```typescript
const revalidateDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(
  null,
);
```

**Then Update the Agent Event Listener** (replace the `loadAssistants()` call):

**Find** (in the agent:event listener we just added):

```typescript
// Refresh assistant list (debounced by React state)
loadAssistants();
```

**Replace With**:

```typescript
// Debounce rapid updates (e.g., bulk operations)
if (revalidateDebounceRef.current) {
  clearTimeout(revalidateDebounceRef.current);
}

revalidateDebounceRef.current = setTimeout(() => {
  loadAssistants();
}, 300); // 300ms debounce
```

**Add Cleanup** (in return statement of same useEffect):

**Find**:

```typescript
return () => {
  if (unlisten) {
    unlisten();
    logger.debug('Agent event listener unregistered');
  }
};
```

**Replace With**:

```typescript
return () => {
  if (unlisten) {
    unlisten();
    logger.debug('Agent event listener unregistered');
  }
  if (revalidateDebounceRef.current) {
    clearTimeout(revalidateDebounceRef.current);
  }
};
```

**Rationale**: Prevents excessive API calls if AI agent creates multiple assistants in quick succession.

**Test Point**: Rapid successive events only trigger one `loadAssistants()` call.

---

## Phase 3: MCP Manager Integration (15 min)

### Step 3.1: Add Event Emission to MCP Manager Operations (15 min)

**File**: `src-tauri/src/mcp/builtin/mcp_manager/operations.rs`

#### 3.1.1: Add Import (Line 1)

**Find**:

```rust
use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, operation_failed_error, ErrorCategory, ErrorGuidance,
    SuccessHint, ToolGroup,
};
```

**Add After**:

```rust
use crate::agent::events;
```

#### 3.1.2: createServer Event Emission (Line 117)

**Find**:

```rust
    server.invalidate_cache().await;

    let hint = SuccessHint::new(
```

**Replace With**:

```rust
    server.invalidate_cache().await;

    // Emit resource updated event for frontend cache revalidation
    events::emit_resource_updated("mcpServer", "create", Some(name.clone()));

    let hint = SuccessHint::new(
```

#### 3.1.3: updateServer Event Emission (Line 175)

**Find**:

```rust
    server.invalidate_cache().await;

    let hint = SuccessHint::new(
```

**Replace With**:

```rust
    server.invalidate_cache().await;

    // Emit resource updated event for frontend cache revalidation
    events::emit_resource_updated("mcpServer", "update", Some(name.to_string()));

    let hint = SuccessHint::new(
```

#### 3.1.4: deleteServer Event Emission (Line 246)

**Find**:

```rust
    server.invalidate_cache().await;

    let hint = SuccessHint::new(
```

**Replace With**:

```rust
    server.invalidate_cache().await;

    // Emit resource updated event for frontend cache revalidation
    events::emit_resource_updated("mcpServer", "delete", Some(name.clone()));

    let hint = SuccessHint::new(
```

#### 3.1.5: verifyServer Event Emission (Line 303)

**Find**:

```rust
    server.invalidate_cache().await;

    Ok(SuccessHint::new(
```

**Replace With**:

```rust
    server.invalidate_cache().await;

    // Emit resource updated event (verify may trigger configuration changes)
    events::emit_resource_updated("mcpServer", "update", Some(name.to_string()));

    Ok(SuccessHint::new(
```

**Test Point**: Compile and runtime verification.

---

## Phase 4: Testing Strategy (30 min)

### Test 4.1: Unit Tests

**File**: `src-tauri/src/agent/events.rs` (add test module)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_updated_event_serialization() {
        let event = AgentEvent::ResourceUpdated {
            resource_type: "assistant".to_string(),
            action: "create".to_string(),
            resource_id: Some("test-id-123".to_string()),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("resourceUpdated"));
        assert!(json.contains("assistant"));
        assert!(json.contains("create"));
    }

    #[test]
    fn test_emit_resource_updated_without_app_handle() {
        // Should not panic when AppHandle not available
        emit_resource_updated("assistant", "create", Some("id".to_string()));
        // No panic = test passed
    }
}
```

---

### Test 4.2: Integration Test Scenarios

#### Scenario 1: UI-Initiated Assistant Creation

```
1. Open AssistantContext
2. Click "Create Assistant" in UI
3. Fill form and submit
4. ✅ Verify: List updates immediately
5. ✅ Verify: No duplicate event handling
```

#### Scenario 2: AI Agent-Initiated Assistant Creation

```
1. Start agent chat session
2. Send message: "Create a new assistant named 'Test Bot'"
3. AI agent calls builtin_assistant__createAssistant
4. ✅ Verify: Backend log shows "Emitting agent event: ResourceUpdated"
5. ✅ Verify: Frontend log shows "Assistant resource updated via agent event"
6. ✅ Verify: Assistant list refreshes and includes "Test Bot"
```

#### Scenario 3: Rapid Bulk Operations

```
1. AI agent creates 5 assistants in quick succession
2. ✅ Verify: Only 1 loadAssistants() call after debounce period
3. ✅ Verify: All 5 assistants appear in list
4. ✅ Verify: No performance degradation
```

#### Scenario 4: MCP Server Updates

```
1. AI agent calls builtin_mcp_manager__createServer
2. ✅ Verify: MCPServerRegistryContext refreshes
3. ✅ Verify: AgentChatStartView reflects new server
```

#### Scenario 5: Error Handling

```
1. Simulate AppHandle not available (early startup)
2. ✅ Verify: emit_resource_updated logs debug message
3. ✅ Verify: No panic, graceful degradation
4. ✅ Verify: UI still works via service layer events
```

---

### Test 4.3: Performance Benchmarks

**Metric 1: Event Emission Overhead**

- Target: < 1ms per event
- Test: Emit 1000 events, measure average time

**Metric 2: Debounce Effectiveness**

- Target: 10 rapid events → 1 API call
- Test: Emit 10 events within 100ms, verify single loadAssistants()

**Metric 3: Memory Usage**

- Target: No memory leaks from event listeners
- Test: Mount/unmount components 100 times, check memory

---

## Phase 5: Edge Cases & Error Handling

### Edge Case 1: AppHandle Not Available

**Scenario**: Builtin tool called before app fully initialized

**Handling**:

```rust
pub fn emit_resource_updated(...) {
    if let Some(app_handle) = crate::state::get_app_handle() {
        // Emit event
    } else {
        log::debug!("AppHandle not available, skipping event");
        // Graceful fallback - UI will update on next manual action
    }
}
```

**Mitigation**:

- Service layer still emits events for UI-initiated actions
- User can manually refresh if needed
- Low probability (AppHandle initialized very early)

---

### Edge Case 2: Event Listener Not Registered

**Scenario**: Context unmounted or never mounted

**Handling**:

- Events are fire-and-forget
- No error if no listeners
- Next mount will fetch fresh data

**Mitigation**: None needed (by design).

---

### Edge Case 3: Concurrent Modifications

**Scenario**: UI and AI agent modify same resource simultaneously

**Handling**:

- Last write wins (database level)
- Both trigger revalidation
- UI fetches latest state

**Mitigation**:

- Debouncing reduces duplicate calls
- Database transactions ensure consistency

---

### Edge Case 4: Network/Database Errors

**Scenario**: loadAssistants() fails after event

**Handling**:

```typescript
const [{ error }, loadAssistants] = useAsyncFn(async () => {
  try {
    // Fetch assistants
  } catch (err) {
    logger.error('Failed to load assistants', err);
    toast.error('Failed to load assistants');
    throw err; // Captured by useAsyncFn
  }
});
```

**Mitigation**:

- Error state displayed to user
- User can manually retry
- Next event will retry automatically

---

## Phase 6: Rollback Plan

### If Critical Issues Found

**Immediate Rollback** (5 min):

```bash
git revert <commit-hash>
pnpm build
pnpm tauri build
```

**Partial Rollback** (10 min):

1. Comment out event emission in operations.rs
2. Remove frontend listeners
3. Keep infrastructure (low risk)
4. Redeploy

**Fallback to Service Layer** (Already exists):

- UI operations already work via RustAssistantService
- Only AI agent operations affected
- User can manually refresh

---

## Phase 7: Documentation Updates

### Update Files:

1. **agents.md**: Document new event system
2. **docs/architecture/agent-workflow-architecture.md**: Add ResourceUpdated events
3. **docs/guides/builtin-tools-development.md**: Event emission guidelines
4. **CHANGELOG.md**: Add entry for v0.4.20

### Example Changelog Entry:

```markdown
## [0.4.20] - 2026-02-04

### Added

- Event-based cache revalidation for AI agent tool operations
- ResourceUpdated events for assistant and MCP server modifications
- Frontend listeners in AssistantContext and MCPServerRegistryContext

### Fixed

- Frontend cache not updating when AI agents create/update/delete assistants
- Frontend cache not updating when AI agents modify MCP server configurations

### Technical

- Extended agent:event system with ResourceUpdated event type
- Added global AppHandle storage for event emission from builtin tools
- Implemented 300ms debouncing for rapid bulk operations
```

---

## Implementation Checklist

### Phase 1: Backend Infrastructure ✓

- [ ] Add AppHandle storage to state.rs
- [ ] Initialize AppHandle in lib.rs setup
- [ ] Add ResourceUpdated event type
- [ ] Create emit_resource_updated helper
- [ ] Add events to assistant operations
- [ ] Verify compilation
- [ ] Test event emission in logs

### Phase 2: Frontend Infrastructure ✓

- [ ] Add TypeScript event types
- [ ] Add listener to AssistantContext
- [ ] Add listener to MCPServerRegistryContext
- [ ] Add imports
- [ ] Implement debouncing
- [ ] Verify TypeScript compilation
- [ ] Test listener registration

### Phase 3: MCP Manager ✓

- [ ] Add events to MCP manager operations
- [ ] Test createServer event
- [ ] Test updateServer event
- [ ] Test deleteServer event
- [ ] Test verifyServer event

### Phase 4: Testing ✓

- [ ] Unit tests pass
- [ ] UI-initiated operations work
- [ ] AI agent operations trigger events
- [ ] Debouncing works correctly
- [ ] Error handling graceful
- [ ] Performance acceptable

### Phase 5: Documentation ✓

- [ ] Update agents.md
- [ ] Update architecture docs
- [ ] Update CHANGELOG.md
- [ ] Add code comments

### Phase 6: Deployment ✓

- [ ] Merge to dev/0.4.0
- [ ] Tag release v0.4.20
- [ ] Build release binaries
- [ ] Test on clean install

---

## Success Criteria

### Functional Requirements

✅ AI agent creates assistant → Frontend list updates  
✅ AI agent updates assistant → Frontend reflects changes  
✅ AI agent deletes assistant → Frontend removes item  
✅ AI agent modifies MCP server → Frontend registry updates  
✅ UI operations still work (no regression)  
✅ Multiple rapid changes handled gracefully

### Non-Functional Requirements

✅ Event emission overhead < 1ms  
✅ No memory leaks from listeners  
✅ Graceful degradation if AppHandle unavailable  
✅ No breaking changes to existing APIs  
✅ Clear error messages for debugging

### User Experience

✅ No manual refresh needed  
✅ No UI flickering or duplicate renders  
✅ No perceived performance impact  
✅ Consistent behavior across all contexts

---

## Timeline

| Phase                  | Duration           | Dependencies         |
| ---------------------- | ------------------ | -------------------- |
| Phase 1: Backend       | 30 min             | None                 |
| Phase 2: Frontend      | 45 min             | Phase 1 complete     |
| Phase 3: MCP Manager   | 15 min             | Phase 1 complete     |
| Phase 4: Testing       | 30 min             | All phases complete  |
| Phase 5: Documentation | 15 min             | Testing complete     |
| **Total**              | **2 hours 15 min** | Sequential execution |

**Recommended Schedule**:

- Day 1 Morning: Phases 1-3 (Implementation)
- Day 1 Afternoon: Phase 4 (Testing)
- Day 2 Morning: Phase 5 (Documentation) + Deployment

---

## Risk Mitigation Matrix

| Risk                    | Probability | Impact   | Mitigation                       |
| ----------------------- | ----------- | -------- | -------------------------------- |
| Event not received      | Low         | High     | Fallback to service layer events |
| AppHandle init fails    | Very Low    | Medium   | Graceful degradation + logging   |
| Memory leak             | Low         | High     | Proper cleanup in useEffect      |
| Performance degradation | Very Low    | Medium   | Debouncing + monitoring          |
| Type mismatch           | Low         | Low      | TypeScript type guards           |
| Breaking changes        | Very Low    | Critical | No API changes, only additions   |

---

## Monitoring & Observability

### Logs to Watch

**Backend (Rust)**:

```
✅ Global AppHandle initialized for event emission
Emitting agent event: ResourceUpdated { resource_type: "assistant", ... }
```

**Frontend (Browser Console)**:

```
Agent event listener registered for assistant updates
Assistant resource updated via agent event {action: "create", resourceId: "..."}
```

### Metrics to Track

1. **Event Emission Rate**: Events/second
2. **Revalidation Frequency**: loadAssistants() calls/minute
3. **Debounce Effectiveness**: Events received vs API calls ratio
4. **Error Rate**: Failed event emissions / Total emissions

---

## Future Enhancements

### Post-MVP Improvements

1. **Playbook Resource Events**
   - Extend to playbook operations
   - Same pattern as assistant/MCP server

2. **Knowledge Base Events**
   - Knowledge item CRUD events
   - Document upload events

3. **Selective Revalidation**
   - Pass resource ID in event
   - Update only specific item (avoid full list refresh)

4. **Optimistic UI Updates**
   - Update UI immediately
   - Revalidate in background
   - Rollback on error

5. **Event Batching**
   - Batch multiple events into single emission
   - Reduce event overhead for bulk operations

---

## Conclusion

This refactoring plan provides a **low-risk, high-value** solution to the cache revalidation problem. By extending the existing event system rather than creating new infrastructure, we:

- ✅ Minimize code changes
- ✅ Reuse proven patterns
- ✅ Maintain backward compatibility
- ✅ Enable future extensibility

**Estimated ROI**:

- Implementation: 2 hours
- Value: Eliminates entire class of bugs (stale cache)
- Maintenance: Minimal (reuses existing infrastructure)

**Ready to implement!** 🚀
