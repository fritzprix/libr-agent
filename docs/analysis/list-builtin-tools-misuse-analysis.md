# Analysis: `listBuiltinTools` Misuse with External MCP Servers

## Issue Summary

The AI agent attempted to use `builtin_mcp_manager__listBuiltinTools` to retrieve tools from the external MCP server `yahoo-finance-mcp`, but consistently received 0 tools despite `verifyServer` confirming 9 tools were available.

**Root Cause:** `listBuiltinTools` is designed exclusively for **builtin LibrAgent services**, not external MCP servers.

## Architecture Overview

### LibrAgent Server Types

LibrAgent has **two distinct types** of MCP servers:

#### 1. **Builtin Services** (Internal LibrAgent Servers)

- **Location:** `src-tauri/src/mcp/builtin/`
- **Examples:** `planning`, `knowledge`, `browser`, `workspace`, `content_store`, `assistant`, `playbook`, `bootstrap`, `ui`, `mcp_manager`
- **Characteristics:**
  - Compiled into LibrAgent binary
  - Always available
  - Tools defined statically in Rust code
  - No external process spawning required
  - Listed by `listBuiltinTools`

#### 2. **External MCP Servers** (User-Registered Servers)

- **Location:** Registered via `registerServer`, stored in MCP configuration
- **Examples:** `yahoo-finance-mcp`, `ddg-search`, `mcp-hn`, `chess-mcp-server`
- **Characteristics:**
  - External processes (stdio) or HTTP services
  - Spawned per-session via `MCPServiceProxyManager`
  - Tools discovered at runtime via MCP protocol
  - Not included in `listBuiltinTools`
  - Listed by `listServers`

## Code Evidence

### `listBuiltinTools` Implementation

```rust
// src-tauri/src/mcp/server/tools.rs:621
pub fn get_static_tools_for_server(server_name: &str) -> Vec<MCPTool> {
    match server_name {
        "planning" => crate::mcp::builtin::planning::PlanningServer::tools_static(),
        "knowledge" => crate::mcp::builtin::knowledge::KnowledgeServer::tools_static(),
        "browser" => crate::mcp::builtin::browser::BrowserServer::tools_static(),
        "workspace" => crate::mcp::builtin::workspace::WorkspaceServer::tools_static(),
        "content_store" | "contentstore" => {
            crate::mcp::builtin::content_store::ContentStoreServer::tools_static()
        }
        "assistant" | "assistant_manager" => {
            crate::mcp::builtin::assistant::AssistantServer::tools_static()
        }
        "playbook" => crate::mcp::builtin::playbook::PlaybookServer::tools_static(),
        "bootstrap" => crate::mcp::builtin::bootstrap::tools::all_tools(),
        "ui" => crate::mcp::builtin::ui::tools::all_tools(),
        "mcp_manager" => crate::mcp::builtin::mcp_manager::tools::all_tools(),
        _ => Vec::new(),  // ❌ External servers return empty vector
    }
}
```

**Key Insight:** The function uses a hardcoded `match` statement that only recognizes builtin server names. Any external server name (like `yahoo-finance-mcp`) returns an empty vector.

### Tool Listing Flow

```rust
// src-tauri/src/mcp/builtin/mcp_manager/queries.rs:203
pub async fn list_builtin_tools(args: Value) -> Result<MCPResult, String> {
    let server_name = args.get("serverName").and_then(|v| v.as_str()).map(|s| s.to_string());

    let tools = if let Some(name) = server_name.as_ref() {
        crate::mcp::server::tools::get_static_tools_for_server(name)  // ✅ Static builtin only
    } else {
        crate::mcp::server::tools::get_all_static_builtin_tools()     // ✅ All builtin only
    };
    // ...
}
```

The tool explicitly calls **static** tool retrieval functions that only work with builtin servers.

### Tool Hint Message

From the trace, the tool response provides a clear hint:

```
💡 Next: Available servers: planning, knowledge, browser, workspace,
         content_store, assistant, playbook, bootstrap, ui, mcp_manager
```

This list **excludes** external servers like `yahoo-finance-mcp`, `ddg-search`, etc.

## Why `verifyServer` Shows 9 Tools

`verifyServer` works differently:

```rust
// src-tauri/src/mcp/builtin/mcp_manager/operations.rs
pub async fn verify_server(args: Value) -> Result<MCPResult, String> {
    // 1. Creates temporary connection to external server
    let temp_proxy = MCPServerManager::global().spawn_temp_connection(name, &config).await?;

    // 2. Calls listTools via MCP protocol (not static definitions)
    let tools = temp_proxy.list_tools().await?;

    // 3. Reports actual tool count from running server
    let tool_count = tools.len();  // Shows 9 tools for yahoo-finance-mcp
}
```

`verifyServer`:

- Spawns the external process
- Communicates via MCP protocol
- Retrieves tools dynamically from the running server
- Returns actual tool count

## Agent Trace Analysis

### What the Agent Did Wrong

1. **Used `listBuiltinTools` for External Server**

   ```
   builtin_mcp_manager__listBuiltinTools(serverName: "yahoo-finance-mcp")
   → Result: 0 tools
   ```

2. **Repeated the Same Mistake Multiple Times**
   - Called `listBuiltinTools` 5+ times
   - Always got 0 tools
   - Did not recognize the pattern

3. **Ignored Tool Hints**
   - Response explicitly listed available builtin servers
   - `yahoo-finance-mcp` was not in the list
   - Agent did not adjust approach

### What the Agent Did Right

1. **Used `verifyServer` Correctly**

   ```
   verifyServer(name: "yahoo-finance-mcp")
   → Result: 9 tools available, server responsive
   ```

2. **Added Server to Assistant Configuration**

   ```
   updateAssistant(
     id: "z675w9jboanqvnfjx0unt6wl",
     mcpServerIds: ["yahoo-finance-mcp"]
   )
   → ✅ Correct approach
   ```

3. **Trusted `verifyServer` Result**
   - Proceeded with assistant update despite `listBuiltinTools` returning 0
   - This was the correct decision

## Correct Workflow for External MCP Servers

### For Users/Agents Adding External Tools

1. **Discovery:**

   ```
   listServers() → Check registered external servers
   searchServer(query: "finance") → Search for specific servers
   ```

2. **Verification:**

   ```
   verifyServer(name: "yahoo-finance-mcp") → Confirm server works and see tool count
   ```

3. **Assignment:**

   ```
   updateAssistant(
     id: "<assistant-id>",
     mcpServerIds: ["yahoo-finance-mcp"]  // ✅ Assign server to assistant
   )
   ```

4. **Tool Access:**
   - Tools become available when the assistant starts a session
   - Tools are loaded dynamically when assistant is used
   - No need to pre-list tools from external servers

### When to Use `listBuiltinTools`

**ONLY for builtin LibrAgent services:**

```
listBuiltinTools() → All 88+ builtin tools
listBuiltinTools(serverName: "workspace") → Workspace-specific tools
listBuiltinTools(serverName: "planning") → Planning-specific tools
```

**NEVER for external servers:**

```
❌ listBuiltinTools(serverName: "yahoo-finance-mcp") → Always returns 0
❌ listBuiltinTools(serverName: "ddg-search") → Always returns 0
```

## Why External Tools Are Not Pre-Listed

### Design Rationale

External MCP servers are **session-scoped** and **dynamically loaded**:

1. **Session Isolation:**
   - Each agent session spawns its own external server processes
   - Tools are discovered when session starts
   - Avoids global state contamination

2. **Lazy Loading:**
   - External servers only spawn when assistant is used
   - Reduces resource consumption
   - Faster startup for app

3. **Dynamic Discovery:**
   - Tool schemas come from running servers
   - Supports servers that modify tools at runtime
   - No need to cache/sync tool definitions

### Implementation Note

From `operations.rs`:

```rust
// External servers are now created per-session through MCPServiceProxyManager
"External servers are managed per-session through MCPServiceProxyManager."
```

## Recommendations

### For Tool Design

1. **Rename Tool for Clarity:**

   ```
   listBuiltinTools → listInternalTools
   ```

   This makes it obvious the tool only works with internal LibrAgent services.

2. **Add Explicit Error Message:**

   ```rust
   pub fn get_static_tools_for_server(server_name: &str) -> Vec<MCPTool> {
       match server_name {
           // ... builtin servers ...
           _ => {
               // Check if it's an external server
               if is_external_server(server_name) {
                   log::warn!("listBuiltinTools called for external server '{}'. \
                              External servers are managed per-session. \
                              Use verifyServer to check tool availability.", server_name);
               }
               Vec::new()
           }
       }
   }
   ```

3. **Enhance Tool Description:**

   ```rust
   MCPTool {
       name: "listBuiltinTools",
       description: "List tools from BUILTIN LibrAgent services only.

       ⚠️ IMPORTANT: This tool does NOT work with external MCP servers.

       BUILTIN SERVICES (supported):
       - planning, knowledge, browser, workspace, content_store
       - assistant, playbook, bootstrap, ui, mcp_manager

       EXTERNAL SERVERS (not supported):
       - yahoo-finance-mcp, ddg-search, mcp-hn, etc.

       For external servers:
       1. Use verifyServer to check tool availability
       2. Add server to assistant via mcpServerIds
       3. Tools load automatically when assistant is used",
       // ...
   }
   ```

### For Agent Instructions

Add to system prompt or agent guidelines:

```markdown
## MCP Server Tool Discovery

**Builtin Services (Internal):**

- Use `listBuiltinTools` to list tools
- Always available, no spawning required
- Examples: workspace, browser, planning

**External Servers (User-Registered):**

- Use `listServers` to see registered servers
- Use `verifyServer` to check if server works and see tool count
- Do NOT use `listBuiltinTools` for external servers
- Add to assistant via `mcpServerIds` field
- Tools load automatically when assistant starts session
```

### For Documentation

Add prominent warning in MCP documentation:

```markdown
## Tool Discovery Differences

| Server Type  | List Tools         | Verify                 | Assign to Assistant            |
| ------------ | ------------------ | ---------------------- | ------------------------------ |
| **Builtin**  | `listBuiltinTools` | N/A (always available) | `allowedBuiltInServiceAliases` |
| **External** | ❌ Not supported   | `verifyServer`         | `mcpServerIds`                 |

⚠️ **Common Mistake:** Using `listBuiltinTools` for external servers always returns 0 tools.
✅ **Correct:** Use `verifyServer` to confirm external server works, then assign via `mcpServerIds`.
```

## Conclusion

**The tool is working as designed.** The issue is a **conceptual misunderstanding** where the agent (and potentially users) expect `listBuiltinTools` to work with external MCP servers.

**Key Takeaways:**

1. ✅ **`listBuiltinTools` is correctly implemented** - it only lists builtin services
2. ✅ **External server tools are not pre-listable by design** - they're session-scoped
3. ✅ **The agent's final approach was correct** - trust `verifyServer`, assign to assistant
4. ⚠️ **Documentation/naming could be clearer** - prevent future confusion

**The tool should NOT be modified to list external tools.** Instead, documentation and error messages should be improved to guide users to the correct workflow.
