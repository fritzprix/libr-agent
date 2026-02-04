# Session Proxy Destroyed After updateAssistant - CRITICAL BUG ANALYSIS

## Executive Summary

**Bug**: After executing `builtin_assistant__updateAssistant`, the entire session proxy is destroyed, causing ALL subsequent builtin tool calls to fail with "Session context not found or expired".

**Severity**: 🔴 **CRITICAL** - Breaks agent workflows that modify assistant configuration  
**Scope**: **ALL builtin tools** (not limited to planning)  
**Impact**: Agent becomes completely non-functional after updating assistant

---

## Bug Manifestation

### Observed Error Sequence

```
Session: dqfs7997en1tn2s5255woz9u

✅ createGoal(title: "Create Investment Analyst Assistant") → Success
✅ addTodo(goalId: ...) → Success
✅ checkTodo(todoId: ...) → Success
✅ updateAssistant(id: ..., mcpServerIds: ["yahoo-finance-mcp"]) → Success

❌ clearGoal() → Session context not found or expired (ID: dqfs7997en1tn2s5255woz9u)
❌ createGoal(...) → Session context not found or expired (ID: dqfs7997en1tn2s5255woz9u)
❌ addTodo(...) → Session context not found or expired (ID: dqfs7997en1tn2s5255woz9u)
❌ listServers() → Session context not found or expired (ID: dqfs7997en1tn2s5255woz9u)
```

### Key Facts

1. **Session ID remains valid** - The same session ID (`dqfs7997en1tn2s5255woz9u`) is used throughout
2. **NOT tool-specific** - ALL builtin tools fail (planning, assistant, mcp_manager, etc.)
3. **Proxy is destroyed** - Error originates from `MCPServiceProxyManager::call_tool()` when it can't find the proxy
4. **No recovery** - Agent cannot continue; requires session restart

---

## Code Analysis

### Error Origin

**File**: `/src-tauri/src/mcp/service_proxy_manager/mod.rs:609-631`

```rust
pub async fn call_tool(
    &self,
    session_id: &str,
    tool_name: &str,
    args: serde_json::Value,
) -> Result<MCPResponse, String> {
    // Builtin tools route through proxy
    if tool_name.starts_with("builtin_") {
        let proxy = self.get_proxy(session_id).await.ok_or_else(|| {
            let active_sessions = futures::executor::block_on(async {
                self.proxies
                    .read()
                    .await
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
            });
            log::error!(
                "No proxy found for session: {}. Active sessions: {:?}",
                session_id,
                active_sessions
            );
            format!("Session context not found or expired (ID: {})", session_id)  // ← ERROR HERE
        })?;
        return proxy.call_tool(tool_name, args).await;
    }
    // ... external tool routing
}
```

**Root Cause**: `self.proxies` HashMap does NOT contain the session_id key after `updateAssistant` completes.

### updateAssistant Implementation

**File**: `/src-tauri/src/mcp/builtin/assistant/operations.rs:279-380`

```rust
pub async fn update_assistant(server: &AssistantServer, args: Value) -> Result<MCPResult, String> {
    let request: UpdateAssistantRequest = serde_json::from_value(args).map_err(|e| {
        log::error!("Failed to parse UpdateAssistantRequest: {}", e);
        format!("Invalid request format: {}", e)
    })?;

    // ... validation and config preparation ...

    match server
        .get_repository()
        .update_assistant(&request.id, Some(name.clone()), Some(config_str))
        .await
    {
        Ok(_) => {
            // Invalidate cache to refresh assistant list
            server.invalidate_cache().await;  // ← Clears AssistantServer cache only

            // Notify frontend
            events::emit_resource_updated("assistant", "update", Some(request.id.clone()));  // ← Emits event

            // ... success response ...
        }
        Err(e) => {
            // ... error handling ...
        }
    }
}
```

### Cache Invalidation

**File**: `/src-tauri/src/mcp/builtin/assistant/mod.rs:51-58`

```rust
pub(crate) async fn invalidate_cache(&self) {
    match self.cache.try_write() {
        Ok(mut cache) => *cache = None,  // Only clears AssistantServer's internal cache
        Err(_) => log::warn!("Failed to invalidate assistant cache - lock contention"),
    }
}
```

**Analysis**: This ONLY clears the `AssistantServer`'s cache (list of assistants). It does NOT touch session proxies.

### Event Emission

**File**: `/src-tauri/src/agent/events.rs:96-118`

```rust
pub fn emit_resource_updated(resource_type: &str, action: &str, resource_id: Option<String>) {
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

**Analysis**: This emits a Tauri event to the frontend. It's a notification mechanism, NOT a command to destroy sessions.

### Frontend Event Handler

**File**: `/src/context/AssistantContext.tsx:412-442`

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
        loadAssistants(); // ← Reloads assistant list from DB
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

**Analysis**: The frontend ONLY refreshes the assistant list. No session destruction happens here.

---

## Investigation Hypothesis

### What's NOT the Cause

❌ **Cache invalidation** - Only clears AssistantServer cache, not session proxies  
❌ **Event emission** - Only notifies frontend, doesn't execute backend logic  
❌ **Frontend handler** - Only refreshes UI, no backend calls that destroy sessions

### What COULD Be the Cause

Based on code analysis, the session proxy could be destroyed by:

1. **Race condition during MCP server reconnection**
   - `AssistantContext` triggers `connectServersFromAssistant` on assistant update
   - This could trigger proxy recreation with same session_id
   - Old proxy might be destroyed during recreation

2. **Cleanup task removing idle sessions**
   - `MCPServiceProxyManager::start_cleanup_task()` runs periodically
   - Could mistakenly identify active session as idle

3. **Workflow termination side effect**
   - `agent/workflow.rs:terminate_workflow()` calls `proxy_manager.destroy_proxy()`
   - Some code path might trigger workflow termination after assistant update

4. **Hidden destroy_proxy call**
   - There might be another code path calling `destroy_proxy()` that's not obvious
   - Could be in async cleanup, error handler, or event handler

---

## Debugging Strategy

### 1. Add Comprehensive Logging

**File**: `/src-tauri/src/mcp/service_proxy_manager/mod.rs`

```rust
pub async fn destroy_proxy(&self, session_id: &str) {
    // ADD: Log stack trace to identify caller
    log::error!("🔥 DESTROY_PROXY CALLED FOR SESSION: {} - STACK TRACE: {:?}",
        session_id,
        std::backtrace::Backtrace::force_capture()
    );

    let proxy_removed = self.proxies.write().await.remove(session_id).is_some();
    // ... rest of implementation
}
```

### 2. Add Lifecycle Logging

**File**: `/src-tauri/src/agent/session_manager.rs`

```rust
pub async fn call_tool(...) {
    log::info!("📞 Tool call: session={}, tool={}", session_id, tool_name);

    let result = self.proxy_manager.call_tool(session_id, tool_name, args).await;

    if let Err(ref e) = result {
        if e.contains("Session context not found") {
            log::error!("❌ SESSION LOST! Last active sessions: {:?}",
                self.proxy_manager.list_sessions().await
            );
        }
    }

    result
}
```

### 3. Monitor Frontend Reconnection

**File**: `/src/context/AssistantContext.tsx`

```typescript
useEffect(() => {
  currentAssistantRef.current = currentAssistant;

  if (debouncedConnectRef.current) {
    clearTimeout(debouncedConnectRef.current);
  }

  if (currentAssistant) {
    logger.warn(
      '🔄 RECONNECTING MCP SERVERS FOR ASSISTANT:',
      currentAssistant.name,
    );

    debouncedConnectRef.current = setTimeout(() => {
      logger.warn('🔄 EXECUTING connectServersFromAssistant');
      connectServersFromAssistant(currentAssistant);
    }, 500);
  }

  // ... rest of code
}, [currentAssistant, connectServersFromAssistant]);
```

### 4. Check for Rapid Session Recreation

**File**: `/src-tauri/src/mcp/service_proxy_manager/mod.rs`

```rust
pub async fn create_proxy(...) -> Result<Arc<MCPServiceProxy>, String> {
    // CRITICAL: Check if already exists (prevent race conditions)
    {
        let proxies = self.proxies.read().await;
        if let Some(existing) = proxies.get(&session_id) {
            log::warn!("⚠️ PROXY ALREADY EXISTS FOR SESSION: {} - RETURNING EXISTING", session_id);
            emit_status("Session services ready", InitializationStatus::Complete);
            return Ok(existing.clone());
        }
    }

    // ADD: Log when new proxy is being created
    log::warn!("🆕 CREATING NEW PROXY FOR SESSION: {}", session_id);

    // ... rest of implementation
}
```

---

## Proposed Fixes

### Option 1: Session Lifecycle Protection

Add a session lock to prevent destruction during active tool calls:

```rust
pub struct MCPServiceProxyManager {
    proxies: Arc<RwLock<HashMap<String, Arc<MCPServiceProxy>>>>,
    active_tool_calls: Arc<RwLock<HashMap<String, usize>>>,  // ← ADD: Count per session
    // ... existing fields
}

pub async fn call_tool(...) -> Result<MCPResponse, String> {
    // Increment active call counter
    {
        let mut active = self.active_tool_calls.write().await;
        *active.entry(session_id.to_string()).or_insert(0) += 1;
    }

    let result = {
        if tool_name.starts_with("builtin_") {
            let proxy = self.get_proxy(session_id).await.ok_or_else(|| {
                format!("Session context not found or expired (ID: {})", session_id)
            })?;
            proxy.call_tool(tool_name, args).await
        } else {
            // ... external tool routing
        }
    };

    // Decrement active call counter
    {
        let mut active = self.active_tool_calls.write().await;
        if let Some(count) = active.get_mut(session_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                active.remove(session_id);
            }
        }
    }

    result
}

pub async fn destroy_proxy(&self, session_id: &str) {
    // CHECK: Prevent destruction if active tool calls exist
    {
        let active = self.active_tool_calls.read().await;
        if let Some(count) = active.get(session_id) {
            if *count > 0 {
                log::error!(
                    "⛔ BLOCKED destroy_proxy for session {} - {} active tool calls",
                    session_id, count
                );
                return;  // ← PREVENT DESTRUCTION
            }
        }
    }

    // ... proceed with destruction
}
```

### Option 2: Disable Frontend Reconnection During Agent Workflow

Prevent `connectServersFromAssistant` from triggering during active agent sessions:

```typescript
// In AgentSessionContext or AgentChatContext
const { connectServersFromAssistant } = useMCPServer();

useEffect(() => {
  if (currentAssistant && !isAgentActive) {
    // ← ADD: Check if agent is running
    connectServersFromAssistant(currentAssistant);
  }
}, [currentAssistant, connectServersFromAssistant, isAgentActive]);
```

### Option 3: Idempotent Proxy Creation

Make `create_proxy` truly idempotent - never destroy existing proxies:

```rust
pub async fn create_proxy(...) -> Result<Arc<MCPServiceProxy>, String> {
    // CRITICAL: Check if already exists
    {
        let proxies = self.proxies.read().await;
        if let Some(existing) = proxies.get(&session_id) {
            log::debug!("Proxy already exists for session: {} - REUSING", session_id);
            return Ok(existing.clone());  // ← Return existing, don't recreate
        }
    }

    // REMOVED: Clean up stale stdio manager
    // Don't remove existing managers during proxy creation

    // ... create new proxy ONLY if none exists
}
```

### Option 4: Add Session Persistence Flag

Mark agent sessions as "persistent" to prevent cleanup:

```rust
pub struct MCPServiceProxy {
    session_id: String,
    persistent: bool,  // ← ADD: Prevent cleanup for agent sessions
    // ... existing fields
}

pub async fn destroy_proxy(&self, session_id: &str) {
    // CHECK: Don't destroy persistent sessions
    {
        let proxies = self.proxies.read().await;
        if let Some(proxy) = proxies.get(session_id) {
            if proxy.persistent {
                log::warn!("⛔ BLOCKED destroy_proxy for persistent session: {}", session_id);
                return;
            }
        }
    }

    // ... proceed with destruction
}
```

---

## Testing Plan

### Test Case 1: Basic Workflow Reproduction

```rust
#[tokio::test]
async fn test_session_persists_after_assistant_update() {
    let manager = setup_test_manager().await;
    let session_id = "test-session";

    // 1. Create session proxy
    let proxy = manager.create_proxy(
        session_id.to_string(),
        vec!["planning".to_string(), "assistant".to_string()],
        vec![],
        None
    ).await.unwrap();

    // 2. Execute planning tool
    let result = manager.call_tool(
        session_id,
        "builtin_planning__createGoal",
        json!({"title": "Test Goal"})
    ).await;
    assert!(result.is_ok());

    // 3. Execute updateAssistant
    let result = manager.call_tool(
        session_id,
        "builtin_assistant__updateAssistant",
        json!({
            "id": "test-assistant",
            "mcpServerIds": ["yahoo-finance-mcp"]
        })
    ).await;
    assert!(result.is_ok());

    // 4. Execute planning tool again (SHOULD NOT FAIL)
    let result = manager.call_tool(
        session_id,
        "builtin_planning__createGoal",
        json!({"title": "Test Goal 2"})
    ).await;

    // ASSERTION: Session should still exist
    assert!(result.is_ok(), "Session should persist after updateAssistant");

    // 5. Verify proxy still exists
    let active_sessions = manager.list_sessions().await;
    assert!(
        active_sessions.contains(&session_id.to_string()),
        "Session {} should still be active. Active sessions: {:?}",
        session_id,
        active_sessions
    );
}
```

### Test Case 2: Concurrent Tool Calls

```rust
#[tokio::test]
async fn test_concurrent_tool_calls_during_update() {
    let manager = Arc::new(setup_test_manager().await);
    let session_id = "concurrent-test";

    manager.create_proxy(
        session_id.to_string(),
        vec!["planning".to_string(), "assistant".to_string()],
        vec![],
        None
    ).await.unwrap();

    let mgr1 = manager.clone();
    let mgr2 = manager.clone();

    // Spawn concurrent tasks
    let task1 = tokio::spawn(async move {
        mgr1.call_tool(
            session_id,
            "builtin_assistant__updateAssistant",
            json!({"id": "test", "mcpServerIds": []})
        ).await
    });

    let task2 = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;  // Slight delay
        mgr2.call_tool(
            session_id,
            "builtin_planning__createGoal",
            json!({"title": "Concurrent Goal"})
        ).await
    });

    let (r1, r2) = tokio::join!(task1, task2);

    // Both should succeed
    assert!(r1.is_ok());
    assert!(r2.is_ok());
}
```

### Test Case 3: Frontend Reconnection Simulation

```rust
#[tokio::test]
async fn test_no_destroy_during_frontend_reconnect() {
    let manager = setup_test_manager().await;
    let session_id = "reconnect-test";

    manager.create_proxy(
        session_id.to_string(),
        vec!["planning".to_string()],
        vec![],
        None
    ).await.unwrap();

    // Simulate updateAssistant
    manager.call_tool(
        session_id,
        "builtin_assistant__updateAssistant",
        json!({"id": "test", "mcpServerIds": ["yahoo-finance-mcp"]})
    ).await.unwrap();

    // Simulate frontend reconnection (calling create_proxy again)
    let result = manager.create_proxy(
        session_id.to_string(),
        vec!["planning".to_string()],
        vec!["yahoo-finance-mcp".to_string()],
        None
    ).await;

    assert!(result.is_ok(), "create_proxy should be idempotent");

    // Original proxy should still work
    let result = manager.call_tool(
        session_id,
        "builtin_planning__createGoal",
        json!({"title": "After Reconnect"})
    ).await;

    assert!(result.is_ok(), "Original proxy should still be functional");
}
```

---

## Priority Actions

1. **🔴 URGENT**: Add stack trace logging to `destroy_proxy()` to identify the caller
2. **🔴 URGENT**: Add session lifecycle logging to track proxy creation/destruction
3. **🟡 HIGH**: Implement Test Case 1 to reproduce the bug reliably
4. **🟡 HIGH**: Investigate frontend `connectServersFromAssistant` reconnection logic
5. **🟢 MEDIUM**: Implement Option 1 (session lifecycle protection) as safeguard
6. **🟢 MEDIUM**: Review all `destroy_proxy()` call sites in codebase

---

## References

- Original bug report: `docs/analysis/list-builtin-tools-bug-analysis.md`
- Agent trace showing session loss after updateAssistant
- Service proxy manager: `src-tauri/src/mcp/service_proxy_manager/mod.rs`
- Assistant operations: `src-tauri/src/mcp/builtin/assistant/operations.rs`
- Frontend assistant context: `src/context/AssistantContext.tsx`
- Workflow termination: `src-tauri/src/agent/workflow.rs`

---

**Date**: 2026-02-05  
**Status**: ACTIVE INVESTIGATION  
**Next Step**: Add comprehensive logging and run reproduction tests
