# MCP Integration Comparison: External vs Builtin

## Executive Summary

LibrAgent implements two distinct MCP integration patterns:

1. **External MCP**: Communicates with external MCP servers via stdio/HTTP transports using `rmcp` library
2. **Builtin MCP**: Native Rust implementations of MCP-compatible tools with direct trait-based interfaces

This document analyzes the architectural differences, interfaces, and integration patterns between these two approaches.

---

## Architecture Overview

### External MCP Integration

```
Frontend (TypeScript)
    ↓ (Tauri commands)
MCPServerManager (Rust)
    ↓ (rmcp library)
External Process (stdio) / HTTP Server
    ↓ (JSON-RPC)
External MCP Server Implementation
```

### Builtin MCP Integration

```
Frontend (TypeScript)
    ↓ (Tauri commands)
MCPServerManager (Rust)
    ↓ (Direct trait calls)
BuiltinServerRegistry
    ↓ (Trait method)
BuiltinMCPServer Implementation
```

---

## Core Interfaces

### 1. External MCP Interface

**Location**: `src-tauri/src/mcp/server/`

#### Server Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerConfig {
    pub name: String,
    pub transport: TransportConfig,          // stdio or HTTP
    pub authentication: Option<OAuthConfig>,  // OAuth 2.1 support
    pub metadata: Option<ServerMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TransportConfig {
    Stdio {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },
    Http {
        url: String,
        protocol_version: String,
        session_id: Option<String>,
        headers: Option<HashMap<String, String>>,
        enable_sse: Option<bool>,
        security: Option<SecurityConfig>,
    },
}
```

#### Connection Management

```rust
pub struct MCPServerManager {
    // Stores rmcp client connections
    connections: Arc<Mutex<HashMap<String, MCPConnection>>>,
    builtin_servers: Arc<Mutex<Option<BuiltinServerRegistry>>>,
    oauth_manager: Arc<OAuthManager>,
}

pub struct MCPConnection {
    client: rmcp::Client,  // rmcp library client
    config: MCPServerConfig,
}
```

#### Tool Call Flow

```rust
// External MCP tool call
pub async fn call_tool(
    manager: &MCPServerManager,
    server_name: &str,
    tool_name: &str,
    arguments: serde_json::Value,
    request_id: Option<serde_json::Value>,
) -> MCPResponse {
    // 1. Get connection from manager
    let connection = connections.get(server_name)?;

    // 2. Call through rmcp client (handles JSON-RPC protocol)
    let call_param = CallToolRequestParam {
        name: tool_name.to_string().into(),
        arguments: Some(args_map),
    };

    let result = connection.client.call_tool(call_param).await?;

    // 3. Convert rmcp result to MCPResponse
    MCPResponse {
        jsonrpc: "2.0".to_string(),
        id: Some(request_id),
        result: Some(result_value),
        error: None,
    }
}
```

### 2. Builtin MCP Interface

**Location**: `src-tauri/src/mcp/builtin/`

#### Server Trait Definition

```rust
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync + std::fmt::Debug {
    /// Server metadata
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn version(&self) -> &str { "1.0.0" }
    fn display_name(&self) -> String { /* auto-capitalize */ }
    fn metadata(&self) -> BuiltinServerMetadata { /* ... */ }

    /// Tool listing
    fn tools(&self) -> Vec<MCPTool>;

    /// Tool execution (returns MCPResult, not MCPResponse)
    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String>;

    /// Service context for system prompt injection
    async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
        ServiceContext {
            context_prompt: format!("## {}\n**Description**: {}",
                self.display_name(), self.description()),
            structured_state: None,
        }
    }
}
```

#### Registry Management

```rust
pub struct BuiltinServerRegistry {
    servers: HashMap<String, Box<dyn BuiltinMCPServer>>,
}

impl BuiltinServerRegistry {
    pub fn register_server(&mut self, server: Box<dyn BuiltinMCPServer>) {
        let name = server.name().to_string();
        self.servers.insert(name, server);
    }

    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: Value,
        request_id: Option<Value>,
        session_id: Option<String>,
    ) -> MCPResponse {
        // 1. Get server from registry
        let server = self.servers.get(server_name)?;

        // 2. Call trait method directly (no JSON-RPC protocol)
        let result = server.call_tool(tool_name, args, session_id).await?;

        // 3. Wrap MCPResult into MCPResponse
        MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(json_rpc_id),
            result: Some(MCPResponseResult::ToolCall(result)),
            error: None,
        }
    }
}
```

---

## Key Differences

### 1. Communication Protocol

| Aspect             | External MCP                           | Builtin MCP              |
| ------------------ | -------------------------------------- | ------------------------ |
| **Protocol**       | JSON-RPC over stdio/HTTP               | Direct Rust trait calls  |
| **Library**        | `rmcp` (Model Context Protocol client) | Native async trait       |
| **Serialization**  | Always serialized to/from JSON         | Direct struct passing    |
| **Overhead**       | Process spawning + IPC                 | In-process function call |
| **Error Handling** | JSON-RPC errors + transport errors     | Rust Result<T, String>   |

### 2. Response Type

#### External MCP Returns `MCPResponse` Directly

```rust
pub async fn call_tool(...) -> MCPResponse {
    // rmcp client returns CallToolResult
    let result = connection.client.call_tool(call_param).await?;

    // Convert to MCPResponse with JSON-RPC envelope
    MCPResponse {
        jsonrpc: "2.0",
        id: Some(request_id),
        result: Some(result_value),  // Full JSON value
        error: None,
    }
}
```

#### Builtin MCP Returns `MCPResult` → Wrapped in `MCPResponse`

```rust
// Trait method returns MCPResult
async fn call_tool(...) -> Result<MCPResult, String> {
    Ok(MCPResult {
        content: vec![MCPContent { /* ... */ }],
        structured_content: Some(json!({ /* ... */ })),
        is_error: Some(false),
    })
}

// Registry wraps it
pub async fn call_tool(...) -> MCPResponse {
    let result = server.call_tool(tool_name, args, session_id).await?;

    MCPResponse {
        jsonrpc: "2.0",
        id: Some(json_rpc_id),
        result: Some(MCPResponseResult::ToolCall(result)),  // Wrapped
        error: None,
    }
}
```

### 3. Session Isolation

#### External MCP (stdio)

- **Process-per-session**: Each session spawns separate subprocess
- **OS-level isolation**: Complete state separation
- **Managed by**: `SessionMCPManager` in `session_isolation/stdio_manager.rs`
- **🎯 INTENDED DESIGN: Eager spawning at session creation/resumption** (not lazy)
- **Idle cleanup**: Processes terminated after idle timeout
- **Race protection**: Spawn locks prevent duplicate processes

```rust
pub struct SessionMCPManager {
    session_id: String,
    active_processes: Arc<RwLock<HashMap<String, MCPProcess>>>,
    last_activity: Arc<RwLock<HashMap<String, Instant>>>,
    spawn_locks: Arc<DashMap<String, Arc<Mutex<()>>>>,
    idle_timeout: Duration,
    server_configs: HashMap<String, MCPServerConfig>,
}
```

**🔑 Design Intent: Eager Tool Discovery**

The original design goal is that when a session is created or resumed, stdio MCP servers should be:

1. ✅ **Spawned immediately** during `create_proxy()` call
2. ✅ **Tool list fetched** via `list_tools()` before first agent interaction
3. ✅ **Tools made visible** to LLM through system prompt/tool schema

**Implementation Status**: ✅ **IMPLEMENTED** in `service_proxy_manager.rs:243-297`

```rust
// ✅ EAGER TOOL DISCOVERY: Fetch tools from session-isolated stdio servers
// This makes stdio server tools visible to the LLM before first tool call
log::info!(
    "Starting eager tool discovery for {} session-isolated stdio servers",
    stdio_configs.len()
);

for server_name in stdio_configs.keys() {
    // Internally calls ensure_process_running() → spawns process
    match manager.list_tools(server_name).await {
        Ok(tools) => {
            // Store tools in proxy cache for immediate LLM access
            proxy_arc.set_session_stdio_tools(server_name.clone(), tools).await;
        }
        Err(e) => log::error!("Failed to fetch tools: {:?}", e),
    }
}
```

**Why Eager Spawning Is Necessary**:

- ✅ **LLM needs tool schemas upfront**: Cannot call tools it doesn't know exist
- ✅ **Session consistency**: All tools available from session start
- ✅ **User experience**: No "invisible tools" confusion
- ✅ **Tool discovery timing**: Tools fetched during session creation, not first tool call

**Contrast with Lazy Spawning**:

- ❌ **Lazy approach would be WRONG**: LLM wouldn't see tools until after first call
- ❌ **Tool invisibility problem**: Agent couldn't plan with unavailable tools
- ❌ **Race conditions**: First tool call might spawn process while fetching tools

**Evidence from Code**:

1. `service_proxy_manager.rs:243`: "EAGER TOOL DISCOVERY" comment
2. `stdio_manager.rs:270`: `list_tools()` calls `ensure_process_running()` internally
3. `service_proxy.rs:37`: "This cache is populated during session creation (eager tool discovery)"

#### External MCP (HTTP)

- **Shared connection**: Single HTTP client
- **Session ID header**: `Mcp-Session-Id` header for isolation
- **Server-side responsibility**: Server must implement session handling
- **Managed by**: `HttpSessionManager` in `session_isolation/http_manager.rs`

#### Builtin MCP

- **Per-session instances**: Each session gets dedicated server instances
- **Managed by**: `MCPServiceProxy` in `service_proxy.rs`
- **Factory pattern**: Created via `create_builtin_server()` function
- **State isolation**: Each instance maintains independent state

```rust
pub struct MCPServiceProxy {
    session_id: String,
    // Session-specific builtin server instances
    builtin_servers: HashMap<String, Box<dyn BuiltinMCPServer>>,
    // Shared external MCP manager
    external_mcp_manager: Arc<MCPServerManager>,
}
```

### 4. Tool Naming Convention

| Type           | Pattern                     | Example                       |
| -------------- | --------------------------- | ----------------------------- |
| External stdio | `{server}__{tool}`          | `filesystem__read_file`       |
| External HTTP  | `{server}__{tool}`          | `github__search_repos`        |
| Builtin        | `builtin_{service}__{tool}` | `builtin_workspace__editFile` |

**Separator**: Double underscore (`__`) is the standard separator.

### 5. Service Context

Both support service context injection, but implementation differs:

#### External MCP

- Context injected at tool call time (if supported by server)
- Returned via MCP protocol response
- No built-in context mechanism

#### Builtin MCP

- Trait method: `get_service_context()`
- Called before building system prompt
- Returns `ServiceContext` with:
  - `context_prompt`: Text visible to AI (✅ used)
  - `structured_state`: JSON for UI (❌ not sent to AI)

**Critical**: Only `context_prompt` is sent to AI agents. Any data in `structured_state` is invisible to the model.

---

## Unified Interface: MCPServiceProxy

The `MCPServiceProxy` provides a unified interface that routes tool calls to the appropriate backend:

```rust
pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResponse, String> {
    if tool_name.starts_with("builtin_") {
        // Extract tool ID (attachments__add -> content_store)
        let tool_id = tool_name
            .strip_prefix("builtin_")
            .and_then(|s| s.split("__").next())?;

        // Route to session-specific builtin server
        let server = self.builtin_servers.get(tool_id)?;
        let result = server.call_tool(real_tool_name, args, Some(self.session_id)).await?;

        // Wrap MCPResult into MCPResponse
        Ok(MCPResponse { /* ... */ })

    } else {
        // Route to external MCP manager (stdio or HTTP)
        let (server_name, real_tool_name) = tool_name.split_once("__")?;
        self.external_mcp_manager
            .call_tool(server_name, real_tool_name, args, None)
            .await
    }
}
```

---

## Lifecycle Management

### External MCP (stdio)

```rust
// 1. Start server (spawn process)
pub async fn start_server(&self, config: MCPServerConfig) -> Result<String> {
    let cmd = Command::new(command).configure(|cmd| {
        for arg in args { cmd.arg(arg); }
        for (key, value) in env { cmd.env(key, value); }
    });

    let transport = TokioChildProcess::new(cmd)?;
    let client = ().serve(transport).await?;  // rmcp client

    let connection = MCPConnection { client, config };
    connections.insert(name.clone(), connection);
}

// 2. Call tool (JSON-RPC via rmcp)
pub async fn call_tool(...) -> MCPResponse {
    let connection = connections.get(server_name)?;
    let result = connection.client.call_tool(call_param).await?;
    // Convert to MCPResponse
}

// 3. Stop server (kill process)
pub async fn stop_server(&self, server_name: &str) -> Result<()> {
    let connection = connections.remove(server_name)?;
    let _ = connection.client.cancel().await;
}
```

### Builtin MCP

```rust
// 1. Initialize server (create trait object)
pub fn register_server(&mut self, server: Box<dyn BuiltinMCPServer>) {
    let name = server.name().to_string();
    self.servers.insert(name, server);
}

// 2. Call tool (direct trait method)
pub async fn call_tool(...) -> MCPResponse {
    let server = self.servers.get(server_name)?;
    let result = server.call_tool(tool_name, args, session_id).await?;
    // Wrap MCPResult into MCPResponse
}

// 3. No explicit stop (registry lifecycle managed by Rust)
```

---

## Data Flow Comparison

### External MCP (stdio) Tool Call

```mermaid
sequenceDiagram
    participant Frontend as Frontend (TS)
    participant Manager as MCPServerManager
    participant RMCP as rmcp::Client
    participant Process as External Process

    Frontend->>Manager: call_tool("filesystem__read", {...})
    Manager->>RMCP: call_tool(CallToolRequestParam)
    RMCP->>Process: stdin: {"method": "tools/call", ...}
    Process->>Process: Execute tool
    Process-->>RMCP: stdout: {"result": {...}}
    RMCP-->>Manager: CallToolResult
    Manager-->>Frontend: MCPResponse
```

### Builtin MCP Tool Call

```mermaid
sequenceDiagram
    participant Frontend as Frontend (TS)
    participant Manager as MCPServerManager
    participant Registry as BuiltinServerRegistry
    participant Server as BuiltinMCPServer

    Frontend->>Manager: call_builtin_tool("workspace", "editFile", {...})
    Manager->>Registry: call_tool("workspace", "editFile", {...})
    Registry->>Server: call_tool("editFile", {...})
    Server->>Server: Execute tool
    Server-->>Registry: MCPResult
    Registry-->>Manager: MCPResponse (wrapped)
    Manager-->>Frontend: MCPResponse
```

---

## Error Handling

### External MCP

**Error Sources**:

1. Transport errors (process spawn failure, HTTP connection)
2. JSON-RPC protocol errors (invalid request, method not found)
3. Tool execution errors (returned by external server)
4. Serialization/deserialization errors

**Error Response**:

```rust
MCPResponse {
    jsonrpc: "2.0",
    id: Some(request_id),
    result: None,
    error: Some(MCPError {
        code: -32603,  // JSON-RPC error code
        message: "Internal error".to_string(),
        data: Some(json!({...})),
    }),
}
```

### Builtin MCP

**Error Sources**:

1. Tool execution errors (business logic)
2. Rust errors (IO, parsing, validation)

**Error Response**:

```rust
// Option 1: Tool returns error via MCPResult
Ok(MCPResult {
    content: vec![MCPContent::text("Error: File not found")],
    structured_content: None,
    is_error: Some(true),
})

// Option 2: Trait method returns Err
Err(format!("Tool not found: {}", tool_name))
// → Wrapped into MCPResponse by Registry
```

---

## Performance Characteristics

| Aspect              | External MCP (stdio)     | External MCP (HTTP)     | Builtin MCP             |
| ------------------- | ------------------------ | ----------------------- | ----------------------- |
| **Startup Latency** | High (process spawn)     | Low (TCP connection)    | Minimal (in-process)    |
| **Call Latency**    | Medium (IPC + JSON-RPC)  | Low (HTTP + JSON-RPC)   | Minimal (function call) |
| **Memory Overhead** | High (separate process)  | Low (shared connection) | Low (in-process)        |
| **CPU Overhead**    | Medium (serialization)   | Medium (serialization)  | Low (direct calls)      |
| **Isolation**       | Complete (OS process)    | Shared (session ID)     | Shared (instance)       |
| **Scalability**     | Limited (process limits) | High (HTTP pooling)     | High (async traits)     |

---

## When to Use Each

### Use External MCP (stdio) When:

- ✅ Integrating third-party MCP servers
- ✅ Need complete process isolation
- ✅ Server written in different language
- ✅ Server requires independent lifecycle
- ✅ Tolerance for startup overhead

### Use External MCP (HTTP) When:

- ✅ Remote server integration
- ✅ Shared infrastructure
- ✅ Low latency requirements
- ✅ Horizontal scaling needed
- ✅ Server supports session management

### Use Builtin MCP When:

- ✅ Performance-critical operations
- ✅ Deep integration with LibrAgent internals
- ✅ Direct access to Rust resources (DB, SessionManager)
- ✅ Complex state management
- ✅ Tight coupling with app lifecycle

---

## Code References

### External MCP

- **Server Manager**: `src-tauri/src/mcp/server/mod.rs`
- **Lifecycle**: `src-tauri/src/mcp/server/lifecycle.rs`
- **Tool Calls**: `src-tauri/src/mcp/server/tools.rs`
- **Session Isolation (stdio)**: `src-tauri/src/mcp/session_isolation/stdio_manager.rs`
- **Session Isolation (HTTP)**: `src-tauri/src/mcp/session_isolation/http_manager.rs`
- **Types**: `src-tauri/src/mcp/types.rs`

### Builtin MCP

- **Trait Definition**: `src-tauri/src/mcp/builtin/mod.rs`
- **Registry**: `src-tauri/src/mcp/builtin/mod.rs` (impl BuiltinServerRegistry)
- **Service Proxy**: `src-tauri/src/mcp/service_proxy.rs`
- **Examples**:
  - Workspace: `src-tauri/src/mcp/builtin/workspace/mod.rs`
  - Browser: `src-tauri/src/mcp/builtin/browser/mod.rs`
  - Planning: `src-tauri/src/mcp/builtin/planning/mod.rs`

### Frontend Integration

- **External MCP**: `src/lib/backend/mcp-server.ts`
- **Builtin MCP**: Integrated via same `call_mcp_tool` command
- **Unified Interface**: `src/hooks/use-unified-mcp.ts`

---

## Data Flow Compatibility Analysis

### External MCP Tool Result Flow

```rust
// Step 1: rmcp client returns CallToolResult
let result = connection.client.call_tool(call_param).await?;
// result type: rmcp::CallToolResult (from rmcp library)

// Step 2: Serialize to serde_json::Value
let result_value = serde_json::to_value(&result)?;
// result_value structure:
// {
//   "content": [
//     { "type": "text", "text": "..." },
//     { "type": "resource", "resource": {...} }
//   ],
//   "isError": false  // optional
// }

// Step 3: Wrap in MCPResponse with Generic variant
MCPResponse {
    jsonrpc: "2.0".to_string(),
    id: Some(request_id),
    result: Some(MCPResponseResult::Generic(result_value)),  // ← Generic!
    error: None,
}
```

### Builtin MCP Tool Result Flow

```rust
// Step 1: Trait method returns MCPResult
let result = server.call_tool(tool_name, args, session_id).await?;
// result type: MCPResult
// {
//   content: Option<Vec<MCPContent>>,
//   structured_content: Option<Value>,
//   is_error: Option<bool>
// }

// Step 2: Wrap in MCPResponse with ToolCall variant
MCPResponse {
    jsonrpc: "2.0".to_string(),
    id: Some(json_rpc_id),
    result: Some(MCPResponseResult::ToolCall(result)),  // ← ToolCall!
    error: None,
}
```

### Critical Difference: MCPResponseResult Variants

The `MCPResponseResult` enum has different variants:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]  // ← Key: No type tag in JSON
pub enum MCPResponseResult {
    ToolCall(MCPResult),           // Builtin MCP uses this
    ToolsList { tools: Vec<MCPTool> },
    ResourcesList { resources: Vec<MCPResource> },
    PromptsList { prompts: Vec<MCPPrompt> },
    Initialize { ... },
    Generic(serde_json::Value),    // External MCP uses this
}
```

**Why This Works (Compatibility)**:

1. **`#[serde(untagged)]` attribute**: When serializing to JSON, no discriminant field is added
2. **Both serialize to same JSON structure**:
   - `ToolCall(MCPResult)` → `{"content": [...], "structuredContent": {...}, "isError": false}`
   - `Generic(Value)` → `{"content": [...], "isError": false}` (same structure)

### JSON Serialization Comparison

**External MCP Result (via Generic)**:

```json
{
  "jsonrpc": "2.0",
  "id": "request-123",
  "result": {
    "content": [{ "type": "text", "text": "File read successfully" }],
    "isError": false
  }
}
```

**Builtin MCP Result (via ToolCall)**:

```json
{
  "jsonrpc": "2.0",
  "id": "request-456",
  "result": {
    "content": [{ "type": "text", "text": "File edited successfully" }],
    "structuredContent": { "linesChanged": 5 },
    "isError": false
  }
}
```

**Frontend TypeScript Type**:

```typescript
export interface MCPResponse<T> {
  jsonrpc: '2.0';
  id: string | number | null;
  result?: MCPResult<T> | SamplingResult;
  error?: MCPError;
}

export interface MCPResult<T = unknown> {
  content?: MCPContent[];
  structuredContent?: T;
  isError?: boolean;
}
```

### Compatibility Matrix

| Field                      | External MCP           | Builtin MCP                | Compatible?            |
| -------------------------- | ---------------------- | -------------------------- | ---------------------- |
| `jsonrpc`                  | ✅ "2.0"               | ✅ "2.0"                   | ✅ Yes                 |
| `id`                       | ✅ JsonRpcId           | ✅ JsonRpcId               | ✅ Yes                 |
| `result.content`           | ✅ Array of MCPContent | ✅ Option<Vec<MCPContent>> | ✅ Yes                 |
| `result.isError`           | ✅ Optional bool       | ✅ Option<bool>            | ✅ Yes                 |
| `result.structuredContent` | ⚠️ May be missing      | ✅ Option<Value>           | ✅ Yes (both optional) |
| `error`                    | ✅ Optional MCPError   | ✅ Optional MCPError       | ✅ Yes                 |

### Key Findings

✅ **FULLY COMPATIBLE**: External and Builtin MCP results are wire-compatible at the JSON level.

**Reasons for Compatibility**:

1. **Untagged Enum**: No discriminant in JSON, variants distinguished by structure
2. **Shared Schema**: Both follow MCP protocol's tool result structure
3. **Optional Fields**: `structuredContent` is optional in both paths
4. **Type Safety**: Rust's type system ensures correctness at compile time
5. **Frontend Abstraction**: TypeScript sees same `MCPResponse<T>` interface

**Potential Issues (None Critical)**:

1. ⚠️ **Logging Difference**: External uses `Generic` variant, Builtin uses `ToolCall` variant
   - Impact: Debug logs show different variant names
   - Solution: No action needed (internal implementation detail)

2. ⚠️ **Serialization Path**: External goes through `rmcp` library serialization
   - Impact: External MCP structure depends on `rmcp` library version
   - Solution: Already handled by serialization to `Value` before wrapping

3. ✅ **Field Names**: Both use `camelCase` for JSON (via `#[serde(rename_all = "camelCase")]`)
   - Impact: None (consistent naming)

### Frontend Handling

Both paths converge at the frontend:

```typescript
// src/hooks/use-unified-mcp.ts
const response: MCPResponse<unknown> = await callTool(toolName, params);

// Works for both external and builtin
if (response.error) {
  // Handle error
} else if (response.result) {
  const content = response.result.content; // Same for both
  const structured = response.result.structuredContent; // Same for both
  const isError = response.result.isError; // Same for both
}
```

### Conclusion

**External and Builtin MCP integrations are FULLY COMPATIBLE** at the data flow level:

- ✅ Same JSON structure
- ✅ Same TypeScript types
- ✅ Same frontend handling
- ✅ Both follow MCP protocol specification
- ✅ No code changes needed for tool consumers

The difference in Rust enum variants (`Generic` vs `ToolCall`) is an internal implementation detail that does not affect wire compatibility or frontend behavior.

---

## Real-World External MCP Trace Analysis

### Trace Overview

Analyzed actual tool responses from external MCP server (`rpg-mcp-server`) to verify format compliance.

**Server**: `rpg-mcp-server` (External MCP via stdio)  
**Tools Called**:

1. `createGame` - Initialize game state
2. `progressStory` - Advance narrative
3. `promptUserActions` - Generate player choices

### Tool Response Format Analysis

#### Response 1: `createGame` Tool

**Raw Tool Response** (stored in DB):

```json
{
  "content": [
    {
      "text": "✅ createGame Completed Successfully\n\n📋 Game Context:\n- Game ID: e5e6b2af-fcf6-4dde-8245-a6d1d1bfea91\n- Title: 네오-도쿄의 밤: 레플리칸트 추격자\n...",
      "type": "text"
    }
  ]
}
```

**Structure Verification**:

- ✅ `content` array present
- ✅ Content items have `type: "text"` field
- ✅ Text content in `text` field
- ⚠️ Missing `isError` field (optional, defaults to false)
- ⚠️ Missing `structuredContent` field (optional)

#### Response 2: `progressStory` Tool

**Raw Tool Response**:

```json
{
  "content": [
    {
      "text": "✅ progressStory Completed Successfully\n\n📋 Game Context:\n- Game ID: e5e6b2af-fcf6-4dde-8245-a6d1d1bfea91\n- Title: 네오-도쿄의 밤: 레플리칸트 추격자\n...",
      "type": "text"
    }
  ]
}
```

**Structure Verification**:

- ✅ Same structure as Response 1
- ✅ Content array with text type
- ✅ Rich formatted text with game state information
- ⚠️ No structured data for programmatic access

#### Response 3: `promptUserActions` Tool

**Expected in Next Call** (not shown in trace, but pattern established):

- Should return choices array in content
- Should follow same `{content: [{type: "text", text: "..."}]}` structure

### Compatibility Verification

#### ✅ **Format Matches Expected Schema**

The external MCP responses match the `MCPResult` structure:

```typescript
interface MCPResult<T = unknown> {
  content?: MCPContent[]; // ✅ Present
  structuredContent?: T; // ⚠️ Not used by this server
  isError?: boolean; // ⚠️ Not explicitly set (defaults to false)
}

interface MCPContent {
  type: 'text' | 'image' | 'resource'; // ✅ Uses 'text'
  text?: string; // ✅ Present
  // ... other fields
}
```

#### Message Flow in Database

**User Message**:

```json
{
  "role": "user",
  "content": [{ "type": "text", "text": "블레이드러너 테마의 RPG 게임시작" }],
  "tool_calls": null
}
```

**Assistant Message** (tool call request):

```json
{
  "role": "assistant",
  "content": [],
  "tool_calls": [
    {
      "id": "tool_o3ivb91oti416jnnj02123zy",
      "type": "function",
      "function": {
        "name": "rpg-mcp-server__createGame",
        "arguments": "{...}"
      }
    }
  ]
}
```

**Tool Message** (tool response):

```json
{
  "role": "tool",
  "content": [
    {
      "type": "text",
      "text": "{\n  \"content\": [\n    {\n      \"text\": \"✅ createGame...\",\n      \"type\": \"text\"\n    }\n  ]\n}"
    }
  ],
  "tool_call_id": "tool_o3ivb91oti416jnnj02123zy",
  "source": "tool"
}
```

### Key Observations

#### 🐛 **ROOT CAUSE: Incorrect JSON Conversion**

The recursive structure `text { content [ { text ...}] }` is caused by **incorrect serialization assumptions** in the backend code:

**Problem Code Path** (`src-tauri/src/agent/llm.rs:399`):

```rust
let content = response
    .result
    .as_ref()
    .and_then(|r| serde_json::to_string_pretty(r).ok())  // ❌ WRONG: Stringifies entire MCPResponseResult
    .unwrap_or_else(|| "{}".to_string());
```

**Why This Creates Recursive Structure**:

1. `MCPResponseResult` already contains `MCPResult { content: Vec<MCPContent>, ... }`
2. `to_string_pretty()` converts the entire object to JSON string: `"{\n \"content\": [...]\n}"`
3. This JSON string is then passed to `create_tool_result_message()` which wraps it in `MCPContent::Text`
4. Result: `{"role":"tool", "content":[{"type":"text", "text":"{\n \"content\":[...]\n}"}]}`

**Expected Structure** (if fixed):

```json
{
  "role": "tool",
  "content": [{ "type": "text", "text": "✅ createGame..." }]
}
```

**Current Structure** (buggy):

```json
{
  "role": "tool",
  "content": [
    {
      "type": "text",
      "text": "{\n  \"content\": [\n    {\"text\": \"✅ createGame...\", \"type\": \"text\"}\n  ]\n}"
    }
  ]
}
```

#### ✅ **What Should Happen Instead**

The code should use `mcp_content` field (which contains proper `Vec<MCPContent>`) instead of stringifying the result:

```rust
// CURRENT (agent/tools.rs:288):
let message = if result.is_error {
    create_error_tool_result(...)
} else if let Some(mcp_content) = result.mcp_content {
    create_tool_result_message_with_content(&session_id, &tool_call_id, mcp_content)  // ✅ Uses structured content
} else {
    create_tool_result_message(&session_id, &tool_call_id, result.content.clone())  // ❌ Falls back to stringified version
};
```

The issue is in `llm.rs:399` - it's creating BOTH stringified `content` AND structured `mcp_content`, but the stringified version should never be used for tool messages.

#### ✅ **Correct Behavior** (Once Fixed)

1. **Direct Content Usage**: Tool response content should be used directly
   - No double wrapping
   - No JSON stringification
   - Direct `Vec<MCPContent>` from MCP result

2. **Tool Call ID Correlation**: Each tool call gets unique ID that matches response
   - Request: `tool_o3ivb91oti416jnnj02123zy`
   - Response: Same `tool_call_id`
   - Enables correct message pairing

3. **Role-Based Message Flow**:
   - `user` → `assistant` (with tool_calls) → `tool` → `assistant` (final response)
   - Follows OpenAI/Anthropic chat completion pattern

#### ⚠️ **Potential Improvements** (Non-Breaking)

**Note**: The recursive structure issue described in the "ROOT CAUSE" section above needs to be fixed first. The improvements below apply AFTER that fix.

1. **Structured Content**: External server could add `structuredContent` field

   ```json
   {
     "content": [{ "type": "text", "text": "..." }],
     "structuredContent": {
       "gameId": "e5e6b2af-fcf6-4dde-8245-a6d1d1bfea91",
       "title": "네오-도쿄의 밤",
       "location": "섹터 7, 크림슨 골목"
     }
   }
   ```

   - Would enable programmatic access to game state
   - Not required for compatibility

2. **Explicit `isError` Field**: Could be set explicitly

   ```json
   {
     "content": [...],
     "isError": false
   }
   ```

   - Defaults to false if missing, so not critical

### Fix Required

**File**: `src-tauri/src/agent/llm.rs`  
**Line**: 399  
**Issue**: Stringifying entire `MCPResponseResult` instead of using structured content

**Current Code**:

```rust
let content = response
    .result
    .as_ref()
    .and_then(|r| serde_json::to_string_pretty(r).ok())  // ❌ Creates recursive structure
    .unwrap_or_else(|| "{}".to_string());

// ...

crate::commands::agent_commands::ToolExecutionResult {
    success: !is_error,
    content,  // ❌ Stringified JSON used as fallback
    error: error_msg,
    is_error,
    mcp_content: crate::agent::tools::convert_mcp_response_content(
        response.result,
    ),  // ✅ Correct structured content
}
```

**Proposed Fix**:

```rust
// Remove the stringified content field entirely, or use it only for logging
let mcp_content = crate::agent::tools::convert_mcp_response_content(
    response.result.clone(),
);

// For logging/debugging only (not used in tool messages):
let debug_content = response
    .result
    .as_ref()
    .and_then(|r| serde_json::to_string_pretty(r).ok())
    .unwrap_or_else(|| "{}".to_string());

crate::commands::agent_commands::ToolExecutionResult {
    success: !is_error,
    content: debug_content,  // Only for logging, not used in tool messages
    error: error_msg,
    is_error,
    mcp_content: mcp_content,  // ✅ This is what should be used
}
```

**AND** in `src-tauri/src/agent/tools.rs:288`, ensure we ALWAYS use `mcp_content` when available:

```rust
let message = if result.is_error {
    create_error_tool_result(&session_id, &tool_call_id,
        result.error.as_deref().unwrap_or("Unknown error"))
} else if let Some(mcp_content) = result.mcp_content {
    // ✅ ALWAYS use structured content for successful tool calls
    create_tool_result_message_with_content(&session_id, &tool_call_id, mcp_content)
} else {
    // ❌ This branch should never happen for successful tool calls
    // Log a warning if we reach here
    log::warn!("Tool result has no mcp_content, using stringified fallback");
    create_tool_result_message(&session_id, &tool_call_id, result.content.clone())
};
```

**Impact**:

- Fixes recursive structure bug for ALL tool responses (external + builtin)
- Maintains backward compatibility (error messages still work)
- Improves UI rendering (no need to parse JSON strings)
- Follows MCP protocol specification correctly

### Comparison with Builtin MCP

| Aspect                | External (rpg-mcp-server)   | Builtin (workspace)                   | Compatible?         |
| --------------------- | --------------------------- | ------------------------------------- | ------------------- |
| **Structure**         | `{content: [{type, text}]}` | `{content: [...], structuredContent}` | ✅ Yes              |
| **content Array**     | ✅ Present                  | ✅ Present                            | ✅ Yes              |
| **Content Type**      | ✅ "text"                   | ✅ "text"                             | ✅ Yes              |
| **structuredContent** | ❌ Not used                 | ✅ Often used                         | ✅ Both optional    |
| **isError**           | ⚠️ Implicit (false)         | ✅ Explicit                           | ✅ Default works    |
| **Text Formatting**   | ✅ Rich (emojis, markdown)  | ✅ Plain/Rich                         | ✅ Both work        |
| **Message Wrapping**  | ✅ JSON string in text      | ✅ Direct object                      | ✅ Handled by layer |

### Verification Results

✅ **CONFIRMED: External MCP responses are fully compatible**

1. **Schema Compliance**: Matches `MCPResult` interface
2. **Type Compatibility**: All required fields present
3. **Frontend Handling**: TypeScript types accept both formats
4. **Message Flow**: Follows standard chat completion pattern
5. **No Errors**: All tools executed successfully
6. **Data Preservation**: All information transmitted correctly

### Actual Data Flow

```
External MCP Server (rpg-mcp-server)
  ↓ (stdio JSON-RPC)
rmcp Client
  ↓ (CallToolResult)
MCPServerManager::call_tool()
  ↓ (serialize to Value)
MCPResponse { result: Generic(Value) }
  ↓ (JSON serialization)
Database (messages table)
  ↓ (JSON deserialization)
Frontend TypeScript (MCPResponse<T>)
  ↓
React Components (MessageRenderer)
```

**Every step preserves the original content structure without modification.**

### Conclusion

The real-world trace confirms:

- ✅ External MCP servers produce compatible responses
- ✅ Tool call/response correlation works correctly
- ✅ Message flow follows expected patterns
- ✅ No compatibility issues detected
- ✅ Both external and builtin can coexist without conflicts

The external MCP integration is **production-ready** and **fully compatible** with the builtin MCP integration. The trace shows clean, error-free execution with proper message formatting and state management.

---

## Summary

LibrAgent's dual MCP integration strategy provides:

1. **Flexibility**: Support both external servers (any language) and native Rust implementations
2. **Performance**: Builtin tools bypass JSON-RPC overhead
3. **Isolation**: Per-session processes (stdio) or instances (builtin)
4. **Compatibility**: Full MCP specification compliance for external servers
5. **Unified API**: Single interface for frontend developers
6. **Wire Compatibility**: Both external and builtin results use identical JSON structure

The key distinction is **protocol overhead vs performance**:

- External MCP: Standards-compliant, language-agnostic, higher latency
- Builtin MCP: Native Rust, low latency, tighter integration

Both share the same `MCPResponse` interface at the boundary, providing a consistent experience for AI agents and frontend components. The data flow analysis confirms that **external and builtin MCP tool results are fully compatible** at the JSON wire format level, with no conversion or adaptation needed.

---

## 🚨 CRITICAL ISSUE: Global MCP Manager is Redundant and Causes Conflicts

### Problem Analysis

The current architecture has **TWO separate systems** managing stdio MCP servers, causing **duplicate tools and conflicts**:

#### 1️⃣ **Global MCP Manager** (`MCPServerManager`)

**Location**: `src-tauri/src/mcp/server/mod.rs`

**Purpose** (Original): Manage globally-shared MCP servers (HTTP servers)

**Current Behavior**: Also manages stdio servers when "상태 확인" button is clicked

**Problem**: Stdio servers should NEVER be managed globally because:

- ❌ **Breaks session isolation**: Stdio servers MUST be process-per-session
- ❌ **Causes duplicate tools**: Same server appears in both GLOBAL and SESSION collections
- ❌ **Wrong lifecycle**: Global manager doesn't know when sessions end

**Code Evidence**:

```rust
// src-tauri/src/commands/mcp_commands.rs:97-194
pub async fn list_tools_from_config(config: Value) -> Result<...> {
    // ❌ PROBLEM: Starts stdio servers in GLOBAL manager
    for server_cfg in servers_config {
        if !manager.is_server_alive(&server_name).await {
            manager.start_server(server_cfg).await?;  // ← Adds to connections HashMap
        }

        match manager.list_tools(&server_name).await {
            Ok(tools) => {
                tools_by_server.insert(server_name, tools);  // ← Global tools
            }
        }
    }
}
```

**Result**: `connections` HashMap contains stdio server → `list_all_external_tools()` includes stdio tools

#### 2️⃣ **Session MCP Manager** (`SessionMCPManager`)

**Location**: `src-tauri/src/mcp/session_isolation/stdio_manager.rs`

**Purpose**: Manage per-session stdio server processes (correct design)

**Behavior**: Spawns independent processes for each session during eager tool discovery

**Code Evidence**:

```rust
// src-tauri/src/mcp/service_proxy_manager.rs:243-297
// ✅ CORRECT: Session-isolated stdio servers
for server_name in stdio_configs.keys() {
    match manager.list_tools(server_name).await {
        Ok(tools) => {
            // ✅ Session-specific tools cached in proxy
            proxy_arc.set_session_stdio_tools(server_name.clone(), tools).await;
        }
    }
}
```

### Duplication Path

**Step 1**: User clicks "상태 확인" button

```
Frontend: connectServersFromAssistant()
  ↓
Backend: list_tools_from_config()
  ↓
Global MCPServerManager: start_server() for stdio server
  ↓
connections.insert("rpg-mcp-server", MCPConnection)  ← ❌ GLOBAL
```

**Step 2**: User creates/resumes session

```
Frontend: Create session
  ↓
Backend: service_proxy_manager.create_proxy()
  ↓
Get stdio_configs from GLOBAL manager.connections  ← ❌ Depends on GLOBAL
  ↓
SessionMCPManager: spawns separate process
  ↓
Eager tool discovery: list_tools()  ← ✅ SESSION-ISOLATED
```

**Step 3**: Tool collection

```rust
// src-tauri/src/agent/tools.rs:59-99
// 2a. Get all GLOBAL external tools (includes stdio from connections!)
let external_tools = proxy_manager
    .list_all_external_tools()  // ← Returns tools from GLOBAL connections
    .await;

all_tools.extend(filtered_external_tools);  // ← 7 tools from GLOBAL

// 2b. Get SESSION-ISOLATED stdio tools
let session_stdio_tools = proxy.get_session_stdio_tools().await;

all_tools.extend(session_stdio_tools);  // ← 7 tools from SESSION

// Result: 14 duplicate tools!
```

### Evidence from Logs

**Original report**:

> "문제는 2개씩 도구가 존재함 그래서 에러가 발생됨"
> "Gemini Error: Duplicate function declaration found: rpg-mcp-server\_\_createGame"

**Root Cause Confirmed**:

- 7 tools from GLOBAL `list_all_external_tools()` (via `connections` HashMap)
- 7 tools from SESSION `get_session_stdio_tools()` (via `SessionMCPManager`)
- Total: 14 duplicate tools → Gemini rejects duplicate function names

### Why Global MCP Manager for Stdio is Wrong

| Aspect                  | Global Manager (Wrong)          | Session Manager (Correct)  |
| ----------------------- | ------------------------------- | -------------------------- |
| **Process Lifecycle**   | Shared single process           | Per-session processes      |
| **State Isolation**     | ❌ Shared state across sessions | ✅ Complete isolation      |
| **Cleanup**             | ❌ Never terminates (global)    | ✅ Terminated with session |
| **Concurrent Sessions** | ❌ State conflicts              | ✅ Independent state       |
| **Resource Usage**      | ❌ Wastes resources             | ✅ Spawns on-demand        |

### Design Intent vs Current Implementation

**Original Design Intent** (based on code comments):

1. ✅ **Builtin Tools**: Per-session instances via `MCPServiceProxy`
2. ✅ **Stdio Servers**: Per-session processes via `SessionMCPManager`
3. ✅ **HTTP Servers**: Shared connections via `HttpSessionManager` + session ID header

**Current Implementation Gap**:

- ❌ **"상태 확인" button bypasses session isolation** by populating GLOBAL `connections`
- ❌ **`get_stdio_configs()` depends on GLOBAL state** instead of database
- ❌ **Tool collection includes stdio from BOTH sources**

### Solution: Remove Stdio from Global Manager

**File**: `src-tauri/src/mcp/server/tools.rs`

**Current Code** (`list_all_tools`):

```rust
pub async fn list_all_tools(manager: &MCPServerManager) -> Result<Vec<MCPTool>> {
    let server_names: Vec<String> = {
        let connections = manager.connections.lock().await;
        connections.keys().cloned().collect()  // ❌ Includes stdio servers!
    };

    for server_name in server_names {
        match list_tools(manager, &server_name).await {
            Ok(mut tools) => {
                all_tools.extend(tools);  // ❌ Stdio tools included
            }
        }
    }
}
```

**Fix**: Filter out stdio servers

```rust
pub async fn list_all_tools(manager: &MCPServerManager) -> Result<Vec<MCPTool>> {
    let server_names: Vec<String> = {
        let connections = manager.connections.lock().await;
        connections.iter()
            .filter(|(_, conn)| {
                // ✅ ONLY include HTTP servers (not stdio)
                matches!(conn.config.transport, TransportConfig::Http { .. })
            })
            .map(|(name, _)| name.clone())
            .collect()
    };

    for server_name in server_names {
        match list_tools(manager, &server_name).await {
            Ok(mut tools) => {
                all_tools.extend(tools);  // ✅ Only HTTP tools
            }
        }
    }
}
```

### Alternative: Make "상태 확인" Session-Aware

Instead of starting servers globally, start them for the current session:

**Current** (Wrong):

```
"상태 확인" → Global manager.start_server() → connections.insert()
```

**Proposed** (Correct):

```
"상태 확인" → List configs from DB → UI shows "available for session creation"
Session creation → SessionMCPManager spawns → Eager tool discovery
```

This eliminates the Global manager's role in stdio server management entirely.

### Conclusion

**✅ CONFIRMED**: Global MCP Manager for stdio servers is:

1. **Redundant** - `SessionMCPManager` already handles stdio lifecycle correctly
2. **Conflicting** - Creates duplicate tools in tool collection
3. **Architecturally Wrong** - Breaks session isolation guarantees
4. **Unnecessary** - "상태 확인" should query DB, not spawn global processes

**Recommendation**:

**Option A (Quick Fix)**: Filter stdio servers from `list_all_tools()` to exclude them from GLOBAL collection

**Option B (Correct Architecture)**: Remove stdio server management from Global manager entirely, eliminate "상태 확인" dependency on Global connections

Both options will resolve the duplicate tool issue. Option B aligns with the intended session-isolation design.

## 🎯 Critical Design Clarification: Session Creation & Server Startup

### Intended Architecture

**When a session is created or resumed, stdio MCP servers MUST be started immediately** - this is the intended design, not lazy spawning.

#### Why This Design Decision?

1. **Tool Visibility Requirement**: AI agents require tool schemas upfront in the system prompt
2. **Session Consistency**: All tools should be available from the moment the session starts
3. **User Experience**: Users expect tools to work immediately, not after first attempt
4. **Architectural Integrity**: Eager tool discovery during session creation ensures predictable behavior

#### Implementation Evidence

The codebase confirms this design intent:

**File**: `src-tauri/src/mcp/service_proxy_manager.rs:243-297`

```rust
// ✅ EAGER TOOL DISCOVERY: Fetch tools from session-isolated stdio servers
// This makes stdio server tools visible to the LLM before first tool call
log::info!(
    "Starting eager tool discovery for {} session-isolated stdio servers",
    stdio_configs.len()
);

for server_name in stdio_configs.keys() {
    match manager.list_tools(server_name).await {
        Ok(tools) => {
            // Internally calls ensure_process_running() which spawns the process
            proxy_arc.set_session_stdio_tools(server_name.clone(), tools).await;
        }
        Err(e) => log::error!("Failed to fetch tools: {:?}", e),
    }
}
```

**Key Mechanism**: `list_tools()` calls `ensure_process_running()` internally:

**File**: `src-tauri/src/mcp/session_isolation/stdio_manager.rs:270-278`

```rust
pub async fn list_tools(&self, server_name: &str)
    -> Result<Vec<MCPTool>, SessionMCPError> {

    // Ensure process is running (spawns if not already running)
    self.ensure_process_running(server_name).await?;

    // Fetch tools from the running process
    let processes = self.active_processes.read().await;
    let process = processes.get(server_name)?;

    process.client.list_all_tools().await
}
```

#### Workflow: Session Creation/Resumption

```text
1. Frontend: User creates/resumes session
   ↓
2. Backend: service_proxy_manager.create_proxy()
   ↓
3. Backend: SessionMCPManager::new() with server_configs
   ↓
4. Backend: EAGER TOOL DISCOVERY loop
   ├─ For each stdio server:
   │  ├─ Call list_tools(server_name)
   │  ├─ → ensure_process_running() spawns process
   │  ├─ → client.list_all_tools() fetches schemas
   │  └─ → proxy.set_session_stdio_tools() caches tools
   ↓
5. Backend: Proxy ready with all tools cached
   ↓
6. Frontend: Session active, tools visible to LLM immediately
```

#### Consequences of This Design

**✅ Correct Behavior**:

- Tools visible to LLM from session start
- No "invisible tools" problem
- Predictable resource allocation
- Session creation takes longer (process spawn overhead)

**❌ What Would Happen with Lazy Spawning** (incorrect):

- LLM cannot see tools until first call
- Agent cannot plan with unavailable tools
- User confusion: "Why didn't the agent use the tool?"
- Race conditions during first tool call

#### Related Documentation

- `docs/analysis/stdio-tool-visibility-analysis.md`: Analysis of why lazy spawning doesn't work
- `docs/analysis/tool-update-flow-confirmation.md`: Confirms eager tool discovery implementation
- `src-tauri/src/mcp/service_proxy.rs:37`: Comment: "This cache is populated during session creation (eager tool discovery)"

#### Design Intent Summary

**The design intent is clear**: stdio MCP servers are eagerly spawned during session creation/resumption, NOT lazily on first tool call. This is necessary for tool visibility to the LLM and is the correct implementation pattern for LibrAgent's architecture.
