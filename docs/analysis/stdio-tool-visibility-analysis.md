# Stdio MCP Server Tool Visibility Analysis

**Date**: 2026-01-19  
**Context**: Understanding when stdio MCP server tools become visible to LLM and how tool calls work with lazy-spawn architecture

---

## Executive Summary

**Key Finding**: 🚨 **CRITICAL TIMING ISSUE IDENTIFIED**

Stdio MCP server tools become visible to the LLM **BEFORE** the server process is spawned. This works because:

1. **Tools are fetched eagerly** when external servers are started via `load_mcp_servers_from_config()`
2. **Tool schemas are cached** in the global `MCPServerManager.connections` HashMap
3. **Stdio processes are spawned lazily** only when the first tool call is made
4. **LLM receives tool schemas** from the cached connection, not from the running process

However, there's a **critical disconnect for stdio servers**:

- **HTTP/SSE servers**: Tools are fetched immediately after connection ✅
- **Stdio servers**: Tools are fetched immediately after spawning, then server keeps running ✅
- **Session-isolated stdio servers**: Process spawned lazily, but NO cached tool schemas ❌

---

## Architecture Overview

### Two MCP Server Types

#### 1. Global External Servers (HTTP/Stdio)

**Managed by**: `MCPServerManager` (singleton, global state)  
**Storage**: `connections: Arc<Mutex<HashMap<String, MCPConnection>>>`  
**Lifecycle**: Started once, shared across all sessions

```rust
pub struct MCPConnection {
    pub client: rmcp::Client<...>,
    pub config: MCPServerConfig,
}
```

#### 2. Session-Isolated Stdio Servers

**Managed by**: `SessionMCPManager` (per-session instances)  
**Storage**: `active_processes: Arc<RwLock<HashMap<String, MCPProcess>>>`  
**Lifecycle**: Lazy-spawn on first tool call, per-session isolation

```rust
pub struct MCPProcess {
    pub client: rmcp::Client<TokioChildProcess>,
    pub active_calls: Arc<AtomicU32>,
}
```

---

## Data Flow: Tool Schema to LLM

### Step 1: External Server Startup (Global)

**Location**: `commands/mcp_commands.rs::load_mcp_servers_from_config()`

```rust
// 1. Frontend calls this command during app initialization
#[tauri::command]
pub async fn load_mcp_servers_from_config() -> Result<HashMap<String, Vec<MCPTool>>, String> {
    let manager = get_mcp_manager();
    let mut tools_by_server: HashMap<String, Vec<MCPTool>> = HashMap::new();

    // 2. Start each external server (HTTP/Stdio)
    for server_cfg in servers_config {
        manager.start_server(server_cfg).await?;  // ⚠️ Spawns stdio process here!

        // 3. Fetch tools immediately after connection
        let tools = manager.list_tools(&server_name).await?;
        tools_by_server.insert(server_name, tools);
    }

    return tools_by_server;  // ✅ Tools cached in frontend + MCPServerManager
}
```

**Key Point**: For global external servers, `start_server()` spawns the stdio process **immediately**, then fetches tools.

---

### Step 2: Session Creation (Per-Session Servers)

**Location**: `mcp/service_proxy_manager.rs::create_proxy()`

```rust
pub async fn create_proxy(
    &self,
    session_id: String,
    server_configs: Vec<MCPServerConfig>,
    builtin_tool_ids: Vec<String>,
) -> Result<(), String> {
    // 1. Filter stdio servers for session isolation
    let stdio_servers: HashMap<String, MCPServerConfig> =
        server_configs.into_iter()
            .filter(|cfg| matches!(cfg.transport, TransportConfig::Stdio { .. }))
            .map(|cfg| (cfg.name.clone(), cfg))
            .collect();

    // 2. Create SessionMCPManager (NO SPAWNING YET!)
    let stdio_manager = SessionMCPManager::new(
        session_id.clone(),
        stdio_servers,  // ✅ Configs stored
        self.config.clone(),
    );

    // 3. Store manager for this session
    self.session_stdio_managers.write().await
        .insert(session_id.clone(), Arc::new(stdio_manager));

    // ❌ NO TOOL FETCHING HAPPENS HERE!
}
```

**Critical Issue**: Session-isolated stdio servers do NOT spawn processes or fetch tools during session creation.

---

### Step 3: LLM Completion Request

**Location**: `agent/llm.rs::request_llm_completion()`

```rust
pub async fn request_llm_completion(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    // 1. Collect available tools for this session
    let available_tools = crate::agent::tools::collect_available_tools(
        &session_id,
        &agent_config,
        proxy_manager,
    ).await.ok();

    // 2. Build completion request with tools
    let request = CompletionRequest {
        session_id,
        messages,
        model,
        provider,
        system_prompt,
        available_tools,  // ✅ Sent to LLM
        // ...
    };

    // 3. Emit to frontend for LLM API call
    app_handle.emit("llm:completion-request", request)?;
}
```

---

### Step 4: Tool Collection Logic

**Location**: `agent/tools.rs::collect_available_tools()`

```rust
pub async fn collect_available_tools(
    session_id: &str,
    agent_config: &crate::agent::AgentConfig,
    proxy_manager: &Arc<MCPServiceProxyManager>,
) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
    let mut all_tools = Vec::new();

    // 1. Collect builtin tools (always available)
    if let Some(proxy) = proxy_manager.get_proxy(session_id).await {
        for tool_id in proxy.builtin_tool_ids() {
            all_tools.extend(proxy.get_builtin_server_tools(&tool_id));
        }
    }

    // 2. Collect external MCP tools ⚠️ THIS IS WHERE THE ISSUE IS!
    if !agent_config.mcp_server_ids.is_empty() {
        // 🚨 Calls MCPServerManager.list_all_tools()
        let external_tools = proxy_manager
            .list_all_external_tools()  // ❌ Only returns GLOBAL server tools!
            .await
            .unwrap_or_default();

        // Filter by allowed server IDs
        let filtered_tools: Vec<_> = external_tools
            .into_iter()
            .filter(|tool| {
                if let Some(server_name) = tool.name.split("__").next() {
                    agent_config.mcp_server_ids.contains(&server_name.to_string())
                } else {
                    false
                }
            })
            .collect();

        all_tools.extend(filtered_tools);
    }

    Ok(all_tools)
}
```

**Critical Analysis**:

```rust
// What proxy_manager.list_all_external_tools() does:
pub async fn list_all_external_tools(&self) -> anyhow::Result<Vec<MCPTool>> {
    // 🚨 Only queries the GLOBAL MCPServerManager!
    self.external_mcp_manager.list_all_tools().await
}

// Inside MCPServerManager.list_all_tools():
pub async fn list_all_tools(manager: &MCPServerManager) -> Result<Vec<MCPTool>> {
    let mut all_tools = Vec::new();

    // 🚨 Only iterates over GLOBAL connections!
    let server_names: Vec<String> = {
        let connections = manager.connections.lock().await;
        connections.keys().cloned().collect()  // ❌ No session-isolated servers here!
    };

    for server_name in server_names {
        let tools = list_tools(manager, &server_name).await?;
        all_tools.extend(tools);
    }

    Ok(all_tools)
}
```

---

## The Missing Link: Session-Isolated Server Tools

### Problem Statement

**Session-isolated stdio servers are NOT included in `collect_available_tools()`!**

```
Agent Configuration:
  mcp_server_ids: ["my-stdio-server"]

Global MCPServerManager:
  connections: {}  // Empty! (server is session-isolated)

SessionMCPManager (per-session):
  server_configs: {"my-stdio-server": {...}}  // Config exists
  active_processes: {}  // Empty! (lazy spawn)

collect_available_tools() result:
  external_tools: []  // ❌ No tools found!

LLM receives:
  available_tools: []  // ❌ LLM doesn't know the server exists!
```

### Why This Is Critical

1. **LLM cannot call tools it doesn't see** in the schema
2. **Session-isolated servers are invisible** to the tool collection logic
3. **Tool schemas are never fetched** for session-isolated servers before first call
4. **Agent will never attempt tool calls** because LLM doesn't know they exist

---

## Solution Paths

### Option 1: Eager Tool Discovery (Recommended)

**Modify**: `mcp/service_proxy_manager.rs::create_proxy()`

```rust
pub async fn create_proxy(
    &self,
    session_id: String,
    server_configs: Vec<MCPServerConfig>,
    builtin_tool_ids: Vec<String>,
) -> Result<(), String> {
    // Existing code...
    let stdio_manager = SessionMCPManager::new(...);

    // ✅ NEW: Eagerly spawn stdio servers and fetch tools
    let mut session_tools: HashMap<String, Vec<MCPTool>> = HashMap::new();
    for (server_name, config) in &stdio_servers {
        // Spawn process
        stdio_manager.ensure_process_running(server_name).await?;

        // Fetch tools via rmcp client
        if let Ok(tools) = stdio_manager.list_tools(server_name).await {
            session_tools.insert(server_name.clone(), tools);
        }
    }

    // Store tools in the proxy
    let proxy = MCPServiceProxy {
        session_id: session_id.clone(),
        stdio_tool_cache: Arc::new(RwLock::new(session_tools)),  // NEW field
        // ...
    };
}
```

**Pros**:

- ✅ Tools available immediately for LLM
- ✅ No changes to lazy-spawn logic for tool calls
- ✅ Process stays alive after tool fetch (no idle timeout yet)

**Cons**:

- ❌ Breaks lazy-spawn optimization (spawns all servers at session start)
- ❌ Increases session creation latency
- ❌ Wastes resources if tools are never called

---

### Option 2: Add Session Tool Accessor (Hybrid Approach)

**Modify**: `agent/tools.rs::collect_available_tools()`

```rust
pub async fn collect_available_tools(
    session_id: &str,
    agent_config: &crate::agent::AgentConfig,
    proxy_manager: &Arc<MCPServiceProxyManager>,
) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
    let mut all_tools = Vec::new();

    // ... builtin tools collection ...

    // 2a. Collect GLOBAL external MCP tools
    let external_tools = proxy_manager
        .list_all_external_tools()
        .await
        .unwrap_or_default();
    all_tools.extend(external_tools);

    // ✅ 2b. Collect SESSION-ISOLATED stdio tools (NEW!)
    if let Some(proxy) = proxy_manager.get_proxy(session_id).await {
        for server_id in &agent_config.mcp_server_ids {
            // Check if this server is session-isolated
            if let Some(tools) = proxy.get_session_stdio_tools(server_id).await {
                all_tools.extend(tools);
            }
        }
    }

    Ok(all_tools)
}
```

**New method in `MCPServiceProxy`**:

```rust
impl MCPServiceProxy {
    /// Get tools from session-isolated stdio server (lazy fetch)
    pub async fn get_session_stdio_tools(
        &self,
        server_name: &str,
    ) -> Option<Vec<MCPTool>> {
        // 1. Check cache first
        if let Some(cached) = self.stdio_tool_cache.read().await.get(server_name) {
            return Some(cached.clone());
        }

        // 2. Fetch from SessionMCPManager (spawns if needed)
        let stdio_managers = self.stdio_managers.read().await;
        if let Some(manager) = stdio_managers.get(&self.session_id) {
            // ✅ This will spawn the process if not running
            if let Ok(tools) = manager.list_tools(server_name).await {
                // Cache for future calls
                self.stdio_tool_cache.write().await
                    .insert(server_name.to_string(), tools.clone());
                return Some(tools);
            }
        }

        None
    }
}
```

**Pros**:

- ✅ Preserves lazy-spawn for actual tool execution
- ✅ Tools spawned only when LLM needs them (first completion request)
- ✅ Cache prevents repeated spawns
- ✅ Minimal code changes

**Cons**:

- ⚠️ First LLM request may timeout if server spawn is slow
- ⚠️ Slightly more complex logic

---

### Option 3: Tool Schema Pre-Registration (No-Spawn)

**Store tool schemas separately from live connections**:

```rust
// In MCPServerConfig, add optional tool schema field:
pub struct MCPServerConfig {
    pub name: String,
    pub transport: TransportConfig,
    pub tool_schemas: Option<Vec<MCPTool>>,  // ✅ NEW: Pre-defined schemas
    // ...
}

// When creating session proxy:
pub async fn create_proxy(
    &self,
    session_id: String,
    server_configs: Vec<MCPServerConfig>,
    builtin_tool_ids: Vec<String>,
) -> Result<(), String> {
    // Extract tool schemas from configs (no spawning!)
    let stdio_tools: HashMap<String, Vec<MCPTool>> =
        server_configs.iter()
            .filter_map(|cfg| {
                cfg.tool_schemas.as_ref().map(|tools| {
                    (cfg.name.clone(), tools.clone())
                })
            })
            .collect();

    // Store in proxy for LLM access
    let proxy = MCPServiceProxy {
        stdio_tool_schemas: Arc::new(RwLock::new(stdio_tools)),
        // ...
    };
}
```

**Pros**:

- ✅ Pure lazy-spawn preserved (no process started until tool call)
- ✅ Zero latency for tool discovery
- ✅ Works even if server is temporarily down

**Cons**:

- ❌ Requires pre-defined schemas (manual maintenance)
- ❌ Schema drift if server changes (no auto-discovery)
- ❌ Not suitable for dynamic MCP servers

---

## Current Behavior Analysis

### What Works Today

1. **Global External Servers** (started via `load_mcp_servers_from_config()`):

   ```
   App Startup → start_server() → spawn process → list_tools() → cache in MCPServerManager
   Session Creation → collect_available_tools() → reads from MCPServerManager cache ✅
   LLM Request → sees all global server tools ✅
   Tool Call → uses existing connection ✅
   ```

2. **Builtin Servers**:
   ```
   App Startup → BuiltinServerRegistry initialized
   Session Creation → builtin tools configured per agent
   LLM Request → collect_available_tools() reads from registry ✅
   Tool Call → executes via BuiltinMCPServer instance ✅
   ```

### What Doesn't Work

3. **Session-Isolated Stdio Servers**:
   ```
   Session Creation → SessionMCPManager created with configs (no spawning)
   LLM Request → collect_available_tools() ❌ MISSES session-isolated servers!
   LLM sees: [] (empty tools)
   LLM cannot call: ANY tools from session-isolated servers
   ```

---

## Recommended Action

**Implement Option 2 (Hybrid Approach)** with the following changes:

### File 1: `src-tauri/src/mcp/service_proxy.rs`

Add method to `MCPServiceProxy`:

```rust
/// Get tools from session-isolated stdio server with lazy fetching
pub async fn get_session_stdio_tools(
    &self,
    server_name: &str,
) -> Result<Vec<MCPTool>, String> {
    // Implementation as shown in Option 2
}
```

### File 2: `src-tauri/src/agent/tools.rs`

Modify `collect_available_tools()`:

```rust
// Add section 2b as shown in Option 2
```

### File 3: Add unit test

```rust
#[tokio::test]
async fn test_session_isolated_tools_visible_to_llm() {
    // Verify session-isolated server tools are included in collection
}
```

---

## Testing Checklist

- [ ] Global external server tools are collected (regression test)
- [ ] Builtin server tools are collected (regression test)
- [ ] Session-isolated stdio server tools are collected (new functionality)
- [ ] Tool cache prevents redundant server spawns
- [ ] Lazy spawn still works for tool execution
- [ ] LLM receives complete tool list including session servers
- [ ] Tool call execution works for all server types

---

## Conclusion

**The current implementation has a critical gap**: Session-isolated stdio servers are invisible to the LLM because their tools are never collected. This breaks the fundamental promise of MCP integration.

**Solution**: Add session-aware tool collection that includes both global and session-isolated servers, with lazy fetching and caching to preserve performance benefits.

**Priority**: 🔴 **HIGH** - This affects core functionality for any agent using session-isolated stdio servers.
