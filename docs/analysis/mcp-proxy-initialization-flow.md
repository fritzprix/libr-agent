# MCP Proxy Initialization Flow Analysis

## Executive Summary

This document traces the full path of MCP proxy initialization during session loading in LibrAgent, from the initial Tauri command through Rust-side proxy creation, event emission, to React-side state consumption. The analysis identifies fragmentation points where HTTP and stdio proxy initialization paths diverge and where state transfer from Rust to React is incomplete.

---

## 1. Full Flow Architecture Map

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         SESSION LOADING ENTRY                          │
├─────────────────────────────────────────────────────────────────────────┤
│  Tauri Command          │  Rust Backend              │  Frontend        │
│                         │                            │                  │
│ agent_create_session    │ ─→ lifecycle::create       │                  │
│ agent_resume_session    │       │                    │                  │
│ agent_open_session      │       ▼                    │                  │
│ agent_send_message      │ ─→ proxy_manager.create    │                  │
│                         │    (singleflight lock)     │                  │
│                         │       │                    │                  │
│                         │       ├─── HTTP Path       │                  │
│                         │       │    │               │                  │
│                         │       │    ▼               │                  │
│                         │       │  HttpSessionMgr    │                  │
│                         │       │  .start_server()   │                  │
│                         │       │     │              │                  │
│                         │       │     ▼              │                  │
│                         │       │  StreamableHTTP     │                  │
│                         │       │  + tool discovery   │                  │
│                         │       │     │              │                  │
│                         │       │     ▼              │                  │
│                         │       │  background         │                  │
│                         │       │  cache update       │                  │
│                         │       │                    │                  │
│                         │       ├─── Stdio Path      │                  │
│                         │       │    │               │                  │
│                         │       │    ▼               │                  │
│                         │       │  SessionMCPMgr     │                  │
│                         │       │  (lazy init)       │                  │
│                         │       │     │              │                  │
│                         │       │     ▼              │                  │
│                         │       │  TokioChildProcess │                  │
│                         │       │     │              │                  │
│                         │       │     ▼              │                  │
│                         │       │  background         │                  │
│                         │       │  cache update       │                  │
│                         │       │                    │                  │
│                         │       ▼                    │                  │
│                         │    Proxy created           │                  │
│                         │    + readiness signal      │                  │
│                         │       │                    │                  │
│                         │       ▼                    │                  │
│                         │    Event emission          │                  │
│                         │    ("agent:event")         │                  │
│                         │       │                    │                  │
│                         │       ▼                    │                  │
│                         │                         React                  │
│                         │                    useAgentSessionEvents       │
│                         │                         │                      │
│                         │                         ▼                      │
│                         │                    ProxyReadiness              │
│                         │                    (watch channel)             │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Entry Points

### 2.1 Session Creation (`agent_create_session`)

**File:** `src-tauri/src/commands/agent_commands.rs:279`
**Path:** `AgentService::create_session` → `AgentSessionManager::create_session` → `lifecycle::create_session`

```rust
// lifecycle/creation.rs:205-213
proxy_manager.create_proxy(session_id, tool_ids, mcp_server_ids, Some(app_handle))
```

### 2.2 Session Resume (`agent_resume_session`)

**File:** `src-tauri/src/commands/agent_commands.rs:289`
**Path:** `AgentSessionManager::resume_session` → `lifecycle::resume_session`

**Key:** Does NOT create new proxy — reuses existing `MCPServiceProxyManager` state.

### 2.3 Session Open (`agent_open_session`)

**File:** `src-tauri/src/commands/agent_commands.rs:298`
**Path:** `AgentSessionManager::get_session`

**Key:** Read-only — loads `SessionMetadata + messages + pendingApprovals` from DB. Does NOT create or check proxy state.

### 2.4 Message Send (`agent_send_message`)

**File:** `src-tauri/src/commands/agent_commands.rs:341`
**Path:** `AgentSessionManager::start_workflow` → `lifecycle::ensure_session_active` → `proxy_manager.create_proxy`

**Key:** Lazy proxy creation if proxy doesn't exist yet.

---

## 3. Proxy Creation Flow

### 3.1 Singleflight Guard

**File:** `src-tauri/src/mcp/service_proxy_manager/creation.rs:177-186`

```rust
// Prevents duplicate proxy creation under concurrent calls
let session_guard = {
    let mut guards = self.creation_guards.lock().await;
    guards.entry(session_id.clone())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
};
```

### 3.2 Config Loading

**File:** `src-tauri/src/mcp/service_proxy_manager/creation.rs:208-285`

1. Loads all MCP server configs from `MCPServerRepository::list()`
2. Filters by `mcp_server_ids` from agent config
3. Splits into `stdio_configs` and `http_configs` HashMaps
4. Returns empty if `mcp_server_ids` is empty (no external servers)

### 3.3 Reuse/Recreate Decision

**File:** `src-tauri/src/mcp/service_proxy_manager/creation.rs:48-75`

```rust
enum ExistingProxyDisposition {
    Reuse,    // Existing proxy matches requested tools/servers
    Recreate, // Configuration mismatch — must rebuild
    Fail,     // Config load failed but tools differ
}
```

### 3.4 HTTP Server Eager Start

**File:** `src-tauri/src/mcp/service_proxy_manager/creation.rs:442-455`

```rust
// HTTP servers connected EAGERLY during proxy creation
if !http_configs.is_empty() {
    emit_status("Connecting to HTTP tool servers", InitializationStatus::Running);
}
for (server_name, config) in &http_configs {
    if let Err(e) = http_manager.start_server(server_name, config.clone()).await {
        log::error!("Failed to start HTTP server {} for session {}: {}", ...);
    }
}
```

**Key:** HTTP connections happen synchronously during `create_proxy`. No "connecting" event granularity per server.

### 3.5 Stdio Server Lazy Start

**File:** `src-tauri/src/mcp/session_isolation/stdio_manager/lifecycle.rs:45`

```rust
// Stdio processes are LAZY — not started during proxy creation
// They are spawned only on first tool call via ensure_process_running()
```

### 3.6 Background Tool Discovery

**File:** `src-tauri/src/mcp/service_proxy_manager/creation.rs:485-530`

```rust
// Proxy is already registered in self.proxies
// Tool discovery runs in background task

let (ready_tx, _) = tokio::sync::watch::channel(false);
self.proxy_readiness.write().await
    .insert(session_id.clone(), ready_tx.clone());

tokio::spawn(async move {
    // Stdio servers: spawned concurrently
    for server_name in stdio_configs_bg.keys() {
        stdio_tasks.spawn(async move {
            // 1. ensure_process_running(server_name)
            // 2. list_all_tools()
            // 3. store in proxy.tool_cache
            // 4. emit InitializationStep events
        });
    }

    // HTTP servers: tool list fetched from already-connected managers
    for server_name in http_configs_bg.keys() {
        // 1. list_all_tools() on HTTP manager
        // 2. store in proxy.tool_cache
    }

    // Wait for ALL tasks to complete
    ready_tx.send_replace(true);  // Signal readiness
});
```

---

## 4. HTTP Proxy Path — Detailed Flow

**File:** `src-tauri/src/mcp/session_isolation/http_manager.rs`

### 4.1 Manager Creation (Eager)

```rust
HttpSessionManager::new(session_id, http_configs)
// Lightweight — no connections, no processes
```

### 4.2 Connection (Eager in create_proxy)

```rust
http_manager.start_server(server_name, config)
```

1. Extracts URL and headers from config
2. Builds `reqwest::Client` with headers
3. Creates `StreamableHttpClientTransport` with `allow_stateless = true`
4. **Does NOT inject `Mcp-Session-Id`** (avoids 400 from standard servers)
5. Calls `().serve(transport)` to establish MCP handshake
6. Updates channel metadata from peer info
7. Stores connection in `connections` HashMap
8. **Spawns background tool cache update**

### 4.3 Tool Discovery (Background)

```rust
spawn_tool_cache_update(
    server_name,
    session_id,
    "HTTP",
    || async {
        client.list_all_tools().await
            .map(|tools| tools.into_iter().map(|t| MCPTool { ... }).collect())
            .map_err(|e| e.to_string())
    },
)
```

---

## 5. Stdio Proxy Path — Detailed Flow

**File:** `src-tauri/src/mcp/session_isolation/stdio_manager/lifecycle.rs`

### 5.1 Manager Creation (Lazy)

```rust
SessionMCPManager::new(session_id, server_configs, config, workspace_dir)
// Lightweight — no processes spawned
```

### 5.2 Process Spawning (Lazy — first tool call only)

```rust
SessionMCPManager::ensure_process_running(server_name)
```

**Race-safe double-checked locking:**

1. Fast path: check if process already in `active_processes` map
2. Acquire per-server spawn lock
3. Double-check: another task may have spawned
4. Extract command/args/env from config
5. **Cross-platform command preparation** (Windows wraps `.cmd/.bat` with `cmd.exe`)
6. **Environment isolation**: `env_clear()` → whitelist system vars → user-defined vars
7. **Process creation**: `TokioChildProcess::new(cmd)`
8. **MCP handshake**: `().serve(transport)` with configurable timeout
9. Update channel metadata, store in `active_processes` map

### 5.3 Error Recovery (Retry)

**File:** `src-tauri/src/mcp/session_isolation/stdio_manager/execution.rs:55-95`

```rust
const MAX_SESSION_RETRIES: usize = 1;

for attempt in 0..=MAX_SESSION_RETRIES {
    let result = self.call_tool_inner(server_name, tool_name, args.clone()).await;
    match result {
        Ok(resp) => return Ok(resp),
        Err(e) if looks_like_session_expired && attempt == 0 => {
            // Reconnect: spawn new process, retry once
            self.reconnect_server(server_name).await?;
        }
        Err(e) => return Err(e),
    }
}
```

### 5.4 Tool Discovery (Background)

Same pattern as HTTP — `spawn_tool_cache_update` after `ensure_process_running` succeeds.

---

## 6. Proxy Object Construction

**File:** `src-tauri/src/mcp/service_proxy/mod.rs:114-163`

```rust
MCPServiceProxy::create(
    session_id,
    tool_ids,           // builtin tool IDs (e.g., "knowledge", "planning")
    db,
    session_manager,
    app_handle,
    http_manager,       // Arc<HttpSessionManager>
    stdio_manager,      // Arc<SessionMCPManager>
    tool_timeout_seconds,
)
```

**Creates:**

- `builtin_servers`: HashMap of `Box<dyn BuiltinMCPServer>` instances
- `session_stdio_tool_cache`: Arc<RwLock<HashMap<String, Vec<MCPTool>>>>
- `session_http_tool_cache`: Arc<RwLock<HashMap<String, Vec<MCPTool>>>>
- `session_managers`: SessionManagers { http, stdio }

**NOT created:**

- Tool caches are empty at construction — populated by background tasks

---

## 7. Event Emission Path (Rust → React)

### 7.1 Dispatcher Architecture

**File:** `src-tauri/src/agent/tauri_events.rs`

```rust
pub struct TauriEventDispatcher {
    app_handle: AppHandle,
}

impl AgentEventDispatcher for TauriEventDispatcher {
    fn emit_agent_event(&self, event: AgentEvent) -> Result<(), String> {
        emit_agent_event(&self.app_handle, event)
    }
}

// Core emission — broadcasts to ALL webviews
pub fn emit_agent_event(app_handle: &AppHandle, event: AgentEvent) -> Result<(), String> {
    app_handle.emit_to(tauri::EventTarget::app(), "agent:event", event)
}
```

### 7.2 Event Types During Initialization

**File:** `src-tauri/src/agent/events.rs:35-40`

```rust
// Emitted during create_proxy for each step
AgentEvent::InitializationStep {
    session_id,
    step: "Connecting to HTTP tool servers" | "Connecting to N stdio servers" | ...,
    status: InitializationStatus::Running | Complete | Error,
}

// Emitted during workflow start (not proxy creation)
AgentEvent::WorkflowStarted { session_id }
AgentEvent::StatusChanged { session_id, status }
AgentEvent::MessageAdded { session_id, message }
```

### 7.3 Events Emitted in create_proxy

| Step              | Event                                                   | Granularity               |
| ----------------- | ------------------------------------------------------- | ------------------------- |
| Config loading    | `InitializationStep: "Connecting to N stdio servers"`   | Per-proxy, not per-server |
| HTTP connection   | `InitializationStep: "Connecting to HTTP tool servers"` | ALL HTTP servers together |
| Tool discovery bg | `InitializationStep` per server (in background task)    | Per-server                |
| Completion        | `InitializationStep: status=complete`                   | Per-proxy                 |

---

## 8. React-Side State Consumption

### 8.1 Main Event Listener

**File:** `src/context/agent-session/useAgentSessionEvents.ts:38`

```typescript
// Mounted during session open (openAgentSession)
unlisten = await listen<AgentEventPayload>('agent:event', (event) => {
    // Filters by sessionId
    switch (payload.type) {
        case 'initializationStep':
            setters.setInitializationStep({
                step: payload.step,
                status: safeStatus,
            });
            break;
        case 'workflowStarted': ...
        case 'statusChanged': ...
        case 'messageAdded': ...
        case 'toolExecutionStarted': ...
        case 'toolExecutionCompleted': ...
    }
});
```

### 8.2 Session Open Flow

**File:** `src/context/agent-session/useAgentSessionEvents.ts:63-132`

```typescript
const initSession = async () => {
  setters.setIsSessionLoading(true);
  setters.setInitializationStep({
    step: 'Starting session...',
    status: 'running',
  });

  unlisten = await listen<AgentEventPayload>('agent:event', handler);

  const response = await openAgentSession(sessionId);
  // response = { session, messages, pendingApprovals }
  // NO proxy state in response!

  setters.setSession(sessionData);
  setters.setIsSessionLoading(false);
};
```

### 8.3 State Setters Available

**File:** `src/context/agent-session/useAgentSessionState.ts`

```typescript
setInitializationStep({ step: string, status: 'running' | 'complete' | 'error' })
setWorkflowStatus('idle' | 'busy' | 'paused' | 'error')
setWorkflowPhase('thinking' | 'answering' | 'using_tools' | 'waiting_approval' | 'idle' | 'error')
setSession(AgentSession)
setMessages(Message[])
setError(string | null)
setIsSessionLoading(boolean)
```

---

## 9. Fragmentation Points

### 9.1 HTTP vs Stdio Initialization Timing Mismatch

| Aspect                | HTTP Path                     | Stdio Path                     |
| --------------------- | ----------------------------- | ------------------------------ |
| **Connection timing** | Eager (during `create_proxy`) | Lazy (first tool call)         |
| **Event emission**    | 1 event for all HTTP servers  | Per-server background events   |
| **Readiness signal**  | Included in background task   | NOT included (lazy!)           |
| **Error handling**    | Logged in `create_proxy`      | Logged in tool execution       |
| **User visibility**   | Progress bar updates          | No visibility until first call |

**Fragmentation:** The `proxy_readiness` watch channel only tracks HTTP tool discovery + stdio tool discovery that runs in background. But stdio processes are LAZY — they never run background discovery unless a tool call happens.

### 9.2 `agent_open_session` Does Not Return Proxy State

**File:** `src-tauri/src/commands/agent_commands.rs:298`

```rust
pub struct AgentOpenSessionResponse {
    pub session: SessionMetadata,
    pub messages: MessageSlice,
    pub pending_approvals: Vec<PendingApprovalSnapshot>,
    // NO proxy state fields!
}
```

**Impact:** React cannot determine if proxy is ready from the open session response. Must rely on:

- `initializationStep` events (which may have already fired before React mounted listener)
- `proxy_readiness` watch channel (but no Tauri command to query it from frontend)

### 9.3 `resume_session` Does Not Emit Readiness Events

**File:** `src-tauri/src/agent/lifecycle/management.rs` (resume_session)

```rust
pub async fn resume_session(...) -> Result<SessionMetadata, String> {
    // Loads session from DB
    // Adds to active_sessions
    // Returns SessionMetadata
    // NO proxy state emission!
}
```

**Impact:** When resuming a session, the React UI shows "session loaded" but doesn't know if proxy is ready. The `useAgentSessionEvents` hook expects `InitializationStep` events during `openAgentSession`, but for resumed sessions, no such events fire.

### 9.4 InitializationStep Event Granularity Coarseness

| Current State                              | Issue                                                                         |
| ------------------------------------------ | ----------------------------------------------------------------------------- |
| `Connecting to HTTP tool servers` (plural) | User doesn't know which server or how many                                    |
| `Connecting to N stdio servers`            | Good count, but no individual progress                                        |
| Background task events                     | Fired AFTER `create_proxy` returns — user may see them on next workflow start |

**Fragmentation:** Events are batched at the proxy level, not at the individual server level. Users see "Connecting to HTTP tool servers" but don't know if it's 1 server or 10, or which ones failed.

### 9.5 No Proxy Readiness Query from React

The `proxy_readiness` is a `tokio::sync::watch::Sender<bool>` stored in Rust. There is NO Tauri command to query it from the frontend.

**Current state in Rust:**

```rust
pub async fn wait_until_proxy_ready(&self, session_id: &str, timeout_secs: u64) -> Result<(), String>
```

This is only callable from Rust-side code (e.g., `start_workflow`). React has no way to:

- Check if proxy is ready
- Query how many servers are connected
- See which servers failed to connect

### 9.6 Error Visibility Gap

| Error Type              | Current Handling                       | User Visibility            |
| ----------------------- | -------------------------------------- | -------------------------- |
| HTTP connection failed  | `log::error` in `create_proxy`         | ❌ Not emitted to React    |
| Stdio spawn failed      | `SessionMCPError` on first tool call   | ⚠️ Only on first tool call |
| Tool discovery timeout  | Background task silently fails         | ❌ No event                |
| Proxy readiness timeout | `wait_until_proxy_ready` returns error | ⚠️ Only in workflow start  |

---

## 10. State Transfer Matrix

| State                | Rust Source                          | Tauri Command  | React Consumer          | Completeness |
| -------------------- | ------------------------------------ | -------------- | ----------------------- | ------------ |
| Session metadata     | `SessionRepository`                  | `open_session` | `useAgentSessionEvents` | ✅ Full      |
| Messages             | `MessageRepository`                  | `open_session` | `useAgentSessionEvents` | ✅ Full      |
| Pending approvals    | `AgentSession.pending_approvals`     | `open_session` | `useAgentSessionEvents` | ✅ Full      |
| Proxy existence      | `MCPServiceProxyManager.proxies`     | ❌ None        | `useAgentSessionEvents` | ❌ None      |
| Proxy readiness      | `proxy_readiness` channel            | ❌ None        | `useAgentSessionEvents` | ❌ None      |
| HTTP server status   | `HttpSessionManager.connections`     | ❌ None        | `useAgentSessionEvents` | ❌ None      |
| Stdio server status  | `SessionMCPManager.active_processes` | ❌ None        | `useAgentSessionEvents` | ❌ None      |
| Tool cache           | `MCPServiceProxy.*_tool_cache`       | ❌ None        | `useAgentSessionEvents` | ❌ None      |
| Initialization steps | `emit_status()` calls                | `agent:event`  | `useAgentSessionEvents` | ⚠️ Partial   |
| Workflow status      | `WorkflowStatus` enum                | `agent:event`  | `useAgentSessionEvents` | ✅ Full      |

---

## 11. Path Fragmentation Summary

### Fragment 1: Initialization Timing Split

- **HTTP:** Connected during `create_proxy` → event fires during open session
- **Stdio:** Connected on first tool call → no initialization event during open session
- **Result:** React sees inconsistent states between sessions with HTTP-only vs stdio servers

### Fragment 2: Proxy State Visibility Gap

- **Rust:** Full visibility of proxy state, server connections, tool caches
- **React:** Only sees `initializationStep` events (coarse granularity) + workflow status
- **Result:** No way to query "how many servers are connected?" from frontend

### Fragment 3: Error Handling Asymmetry

- **HTTP errors:** Logged in Rust, never emitted to React
- **Stdio errors:** Emitted only when first tool call fails (late visibility)
- **Result:** Silent failures for HTTP servers, visible failures for stdio

### Fragment 4: Resume Session State Loss

- **Create session:** `initializationStep` events → React shows progress
- **Resume session:** No events → React shows instant "loaded" state
- **Result:** Inconsistent UX between first-open and resume

---

## 12. Recommendations

### P0: Add Proxy Readiness Query Command

Create `agent_check_proxy_state(session_id)` Tauri command that returns:

```typescript
interface ProxyStateResponse {
  exists: boolean;
  ready: boolean;
  httpServers: { name: string; connected: boolean }[];
  stdioServers: { name: string; connected: boolean; processId?: number }[];
  toolCache: { server: string; toolCount: number }[];
}
```

### P0: Emit Proxy Connection Events

During `create_proxy`, emit per-server events:

```rust
AgentEvent::ServerConnecting { session_id, server_name, transport }
AgentEvent::ServerConnected { session_id, server_name }
AgentEvent::ServerConnectionFailed { session_id, server_name, error }
```

### P1: Unify Resume Session Events

When resuming, emit `InitializationStep` events matching the create flow so React shows consistent loading state.

### P1: Add HTTP Error Events

When `http_manager.start_server` fails, emit an error event instead of only logging.

### P2: Add Tool Discovery Events

```rust
AgentEvent::ToolsDiscovered { session_id, server_name, tool_count }
AgentEvent::ToolsDiscoveryFailed { session_id, server_name, error }
```

### P2: Connect `proxy_readiness` to `initializationStep`

Instead of using a separate watch channel, emit `InitializationStep { step: "Tool discovery complete", status: "complete" }` when the background task finishes, and have React use this as the proxy readiness indicator.

---

## Appendix A: File Reference Map

| Component          | File Path                                                        |
| ------------------ | ---------------------------------------------------------------- |
| Tauri Commands     | `src-tauri/src/commands/agent_commands.rs`                       |
| Session Manager    | `src-tauri/src/agent/session_manager.rs`                         |
| Lifecycle Creation | `src-tauri/src/agent/lifecycle/creation.rs`                      |
| Event Types        | `src-tauri/src/agent/events.rs`                                  |
| Event Dispatcher   | `src-tauri/src/agent/tauri_events.rs`                            |
| Proxy Manager      | `src-tauri/src/mcp/service_proxy_manager/mod.rs`                 |
| Proxy Creation     | `src-tauri/src/mcp/service_proxy_manager/creation.rs`            |
| Proxy Management   | `src-tauri/src/mcp/service_proxy_manager/management.rs`          |
| Proxy Object       | `src-tauri/src/mcp/service_proxy/mod.rs`                         |
| Proxy Builder      | `src-tauri/src/mcp/service_proxy/builder.rs`                     |
| Proxy Factory      | `src-tauri/src/mcp/service_proxy/factory.rs`                     |
| HTTP Manager       | `src-tauri/src/mcp/session_isolation/http_manager.rs`            |
| Stdio Manager      | `src-tauri/src/mcp/session_isolation/stdio_manager/`             |
| Stdio Lifecycle    | `src-tauri/src/mcp/session_isolation/stdio_manager/lifecycle.rs` |
| Stdio Execution    | `src-tauri/src/mcp/session_isolation/stdio_manager/execution.rs` |
| React Events Hook  | `src/context/agent-session/useAgentSessionEvents.ts`             |
| React State Hook   | `src/context/agent-session/useAgentSessionState.ts`              |
| React Types        | `src/context/agent-session/types.ts`                             |

---

_Analysis complete. All file paths verified against workspace._
