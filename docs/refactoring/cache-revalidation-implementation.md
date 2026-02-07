# Cache Revalidation Implementation Summary

**Date**: 2025-01-XX  
**Status**: ✅ Completed  
**Objective**: Enable frontend cache revalidation when AI agents use builtin tools to update assistants or MCP servers

## Problem Statement

When AI agents use builtin tools (via `builtin_assistant__*` or `builtin_mcpServer__*`), the operations bypass the frontend service layer. Backend cache invalidation occurs (`invalidate_cache().await`), but the frontend contexts (AssistantContext, MCPServerRegistryContext) remain unaware, resulting in stale UI data.

**Example Scenario**:

1. Agent calls `builtin_assistant__updateAssistant` to modify system prompt
2. Backend updates SQLite database
3. Backend invalidates in-memory cache
4. Frontend still shows old data until manual page refresh

## Solution Architecture

Extended the existing `agent:event` system to emit `ResourceUpdated` events after backend cache invalidation, allowing frontend contexts to listen and refresh their data.

### Design Principles

- ✅ **Reuse Existing Infrastructure**: Leveraged `agent:event` channel instead of creating new mechanisms
- ✅ **Minimal Changes**: Focused on strategic points (cache invalidation sites)
- ✅ **Event-Driven**: Decoupled backend operations from frontend state management
- ✅ **Debouncing**: Implemented 300ms debounce to prevent excessive refreshes
- ✅ **Type Safety**: Strict TypeScript types with discriminated union

## Implementation Details

### Phase 1: Backend Infrastructure

**Files Modified**:

- `src-tauri/src/state.rs`
- `src-tauri/src/agent/events.rs`
- `src-tauri/src/lib.rs`

**Changes**:

1. **Global AppHandle Storage** (`state.rs`):

   ```rust
   static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

   pub fn init_app_handle(handle: AppHandle) {
       APP_HANDLE.set(handle).expect("AppHandle already initialized");
   }

   pub fn get_app_handle() -> Option<&'static AppHandle> {
       APP_HANDLE.get()
   }
   ```

2. **ResourceUpdated Event Type** (`events.rs`):

   ```rust
   pub enum AgentEvent {
       // ... existing events
       ResourceUpdated {
           resource_type: String,  // "assistant" | "mcpServer"
           action: String,          // "create" | "update" | "delete" | "verify"
           resource_id: Option<String>,
       },
   }
   ```

3. **Helper Function** (`events.rs`):

   ```rust
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
           let _ = app_handle.emit("agent:event", event);
       }
   }
   ```

4. **AppHandle Initialization** (`lib.rs`):
   ```rust
   .setup(|app| {
       // ... existing setup
       crate::state::init_app_handle(app.handle().clone());
       Ok(())
   })
   ```

### Phase 2: Assistant Operations Integration

**Files Modified**:

- `src-tauri/src/mcp/builtin/assistant/operations.rs`

**Changes**:
Added event emissions after cache invalidation in:

- `createAssistant()` → `emit_resource_updated("assistant", "create", Some(name))`
- `updateAssistant()` → `emit_resource_updated("assistant", "update", Some(name))`
- `deleteAssistant()` → `emit_resource_updated("assistant", "delete", Some(name))`

**Pattern**:

```rust
// Invalidate backend cache
server.invalidate_cache().await;

// Emit resource updated event for frontend cache revalidation
events::emit_resource_updated("assistant", "create", Some(name.to_string()));
```

### Phase 3: MCP Manager Operations Integration

**Files Modified**:

- `src-tauri/src/mcp/builtin/mcp_manager/operations.rs`

**Changes**:
Added event emissions after cache invalidation in:

- `createServer()` → `emit_resource_updated("mcpServer", "create", Some(name))`
- `updateServer()` → `emit_resource_updated("mcpServer", "update", Some(name))`
- `deleteServer()` → `emit_resource_updated("mcpServer", "delete", Some(name))`
- `verifyServer()` → `emit_resource_updated("mcpServer", "verify", Some(name))`

### Phase 4: Frontend Type Definitions

**Files Modified**:

- `src/context/AgentSessionContext.tsx`

**Changes**:
Extended `AgentEventPayload` union type:

```typescript
export type AgentEventPayload =
  | { type: 'workflowStarted'; sessionId: string }
  // ... other event types
  | {
      type: 'resourceUpdated';
      resourceType: string;
      action: string;
      resourceId?: string;
    };
```

**Session ID Handling**:
Updated event filter to handle global events:

```typescript
// Allow resourceUpdated events (global) to pass through
if (
  payload.type !== 'resourceUpdated' &&
  'sessionId' in payload &&
  payload.sessionId !== sessionId
) {
  return;
}
```

### Phase 5: Frontend Event Listeners

**Files Modified**:

- `src/context/AssistantContext.tsx`
- `src/context/MCPServerRegistryContext.tsx`

**Changes**:
Added `agent:event` listeners with debouncing:

```typescript
// Subscribe to agent:event for AI agent resource updates (with debouncing)
useEffect(() => {
  let debounceTimer: NodeJS.Timeout | null = null;

  const unlisten = listen<AgentEventPayload>('agent:event', (event) => {
    const payload = event.payload;
    if (
      payload.type === 'resourceUpdated' &&
      payload.resourceType === 'assistant'
    ) {
      logger.debug(
        'Agent updated assistant resource, debouncing refresh...',
        payload,
      );

      // Clear existing timer
      if (debounceTimer) {
        clearTimeout(debounceTimer);
      }

      // Set new timer
      debounceTimer = setTimeout(() => {
        logger.debug('Debounce complete, refreshing assistants...');
        loadAssistants();
      }, 300);
    }
  });

  return () => {
    if (debounceTimer) {
      clearTimeout(debounceTimer);
    }
    unlisten.then((fn) => fn());
  };
}, [loadAssistants]);
```

**Debounce Strategy**:

- 300ms delay to batch rapid updates
- Timer reset on each new event
- Cleanup on unmount

## Event Flow Diagram

```
┌─────────────────────┐
│  AI Agent Workflow  │
│ (Rust Backend Loop) │
└──────────┬──────────┘
           │ Calls builtin tool
           ▼
┌─────────────────────┐
│ Assistant/MCP Tool  │
│  - Update database  │
│  - Invalidate cache │
│  - Emit event ──────┼─────┐
└─────────────────────┘     │
                             │ agent:event
                             ▼
            ┌────────────────────────────┐
            │   Tauri Event System       │
            └────────────────────────────┘
                             │
                ┌────────────┼────────────┐
                │                         │
                ▼                         ▼
    ┌───────────────────┐     ┌───────────────────┐
    │ AssistantContext  │     │ MCPServerRegistry │
    │   - Filter type   │     │    - Filter type  │
    │   - Debounce      │     │    - Debounce     │
    │   - Refresh data  │     │    - Refresh data │
    └───────────────────┘     └───────────────────┘
                │                         │
                ▼                         ▼
         ┌──────────────────────────────────┐
         │      UI Re-renders with         │
         │      Updated Data                │
         └──────────────────────────────────┘
```

## Testing Verification

### Manual Test Cases

1. **Assistant Update via Agent**:
   - Start agent workflow
   - Agent calls `builtin_assistant__updateAssistant`
   - Verify UI refreshes without manual reload
   - Check browser console for debounce logs

2. **MCP Server Creation via Agent**:
   - Agent calls `builtin_mcpServer__createServer`
   - Verify server appears in UI immediately
   - Confirm no duplicate API calls (debouncing working)

3. **Rapid Updates**:
   - Agent makes multiple quick updates
   - Verify debouncing (only one refresh at end)
   - Check logs for timer reset behavior

### Validation Results

✅ **Backend Compilation**: `cargo check` - Pass  
✅ **Frontend TypeScript**: `pnpm tsc --noEmit` - Pass  
✅ **ESLint**: `pnpm lint` - Pass  
✅ **Prettier**: `pnpm format` - Pass  
✅ **Rust Formatting**: `cargo fmt --check` - Pass  
✅ **Clippy**: `cargo clippy -- -D warnings` - Pass

## Risk Assessment

### Low Risk Factors

- ✅ Reuses proven `agent:event` infrastructure
- ✅ Non-breaking additive changes
- ✅ Graceful fallback (no AppHandle = silent skip)
- ✅ Type-safe discriminated union
- ✅ Debouncing prevents performance issues

### Potential Concerns

- ⚠️ **Race Conditions**: Backend update → event emission → frontend refresh → backend still updating
  - **Mitigation**: Database operations are synchronous before emission
- ⚠️ **Memory Leaks**: Debounce timers not cleaned up
  - **Mitigation**: Cleanup in useEffect return function
- ⚠️ **Event Storms**: Rapid agent operations
  - **Mitigation**: 300ms debounce coalesces events

## Performance Impact

### Expected Overhead

- **Event Emission**: ~1ms per operation (async fire-and-forget)
- **Frontend Debounce**: 300ms delay before refresh (user-imperceptible)
- **API Calls**: Same as manual refresh (no additional load)

### Optimization Notes

- Events only emitted on actual changes (not on read operations)
- Debouncing prevents N rapid updates from causing N API calls
- No polling or continuous background processes

## Future Enhancements

### Potential Improvements

1. **Selective Refresh**: Include `resource_id` in event to avoid full list reload
2. **Optimistic UI**: Update local state immediately, reconcile on event
3. **WebSocket Alternative**: Bidirectional communication for real-time updates
4. **Event Batching**: Combine multiple resource updates into single event

### Extensibility

The pattern can be extended to other resources:

- Knowledge store updates
- Planning state changes
- Browser automation session events

## Conclusion

Successfully implemented frontend cache revalidation for AI agent builtin tool operations. The solution:

- ✅ Solves the core problem (stale UI after agent operations)
- ✅ Maintains architectural consistency
- ✅ Passes all validation checks
- ✅ Introduces minimal complexity
- ✅ Provides foundation for future event-driven features

**Next Steps**:

1. Manual testing in development environment
2. Monitor logs for debounce behavior
3. User acceptance testing with real agent workflows
4. Consider adding metrics/telemetry for event flow
