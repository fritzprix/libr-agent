# Bug Analysis: `listBuiltinTools` Silent Failure with External Servers

## Executive Summary

**Bug Confirmed:** `listBuiltinTools` has a **silent failure bug** when called with external MCP server names. It returns 0 tools without error messages or warnings, causing confusion for AI agents and users.

## Bugs Identified

### Bug #1: Silent Failure with Invalid Server Names

**Current Behavior:**

```rust
// queries.rs:211
let tools = if let Some(name) = server_name.as_ref() {
    crate::mcp::server::tools::get_static_tools_for_server(name)  // Returns [] for unknown names
} else {
    crate::mcp::server::tools::get_all_static_builtin_tools()
};
```

```rust
// tools.rs:621
pub fn get_static_tools_for_server(server_name: &str) -> Vec<MCPTool> {
    match server_name {
        "planning" => /* ... */,
        "workspace" => /* ... */,
        // ...
        _ => Vec::new(),  // ❌ Silent failure - no error, no warning
    }
}
```

**Problem:** When `yahoo-finance-mcp` (external server) is passed:

1. Function returns empty vector `Vec::new()`
2. No error is raised
3. No warning is logged
4. Response message: `"Found 0 tools from 'yahoo-finance-mcp' server"` - looks like valid result
5. Agent has no way to know this is an invalid parameter

**Impact:**

- Agent repeats the same call 5+ times
- Wastes tokens and time
- Creates confusion about why external server "has no tools"
- Contradicts `verifyServer` which shows 9 tools

### Bug #2: Misleading Tool Description

**Current Description:**

```rust
// tools.rs:318
description: "List all available built-in MCP tool schemas across all servers.

Use serverName parameter to filter by specific server (e.g., 'planning', 'browser', 'workspace').

Available servers: planning, knowledge, browser, workspace, contentstore,
assistant_manager, playbook, bootstrap, ui, mcp_manager"
```

**Problems:**

1. Says "all available built-in MCP tool schemas" - unclear that external servers are excluded
2. Lists valid server names at END of description - easy to miss
3. No explicit warning: "This tool does NOT work with external servers"
4. Parameter description doesn't mention validation

### Bug #3: Non-Contextual Hint Messages

**Current Implementation:**

```rust
// queries.rs:287-298
let hints = if total_count > PAGE_SIZE {
    vec![
        format!("Available servers: {}", available_servers.join(", ")),
        "Use serverName parameter to filter (e.g., serverName='planning')".to_string(),
        format!("Showing {}/{} tools", PAGE_SIZE, total_count),
    ]
} else {
    vec![
        format!("Available servers: {}", available_servers.join(", ")),
        "Use serverName parameter to filter tools by server".to_string(),
    ]
};
```

**Problem:** Hints are shown for ALL responses, regardless of whether:

- User provided a server name
- The server name was valid
- The server name was an external server

**Better Approach:** Contextual hints based on input:

- If no server name provided → show available servers
- If invalid server name → explain error and show valid servers
- If external server name → explain external servers not supported + suggest `verifyServer`
- If valid server name → show tools

### Bug #4: Missing Validation Against External Server Registry

**Current Code:** No check against registered external servers

**Should Check:**

```rust
// Check if the server name matches a registered external server
let external_configs = list_all_configs().await?;
let is_external = external_configs.iter().any(|cfg| {
    cfg.name.as_ref().map(|n| n == name).unwrap_or(false)
});

if is_external {
    return Err(format!(
        "Server '{}' is an external MCP server. \
         This tool only lists builtin LibrAgent services.\n\n\
         To check external server tools:\n\
         1. Use verifyServer(name: '{}') to see tool count\n\
         2. Add to assistant via updateAssistant(mcpServerIds: ['{}'])\n\
         3. Tools load automatically when assistant starts",
        name, name, name
    ));
}
```

## Evidence from Trace

### Agent Behavior Pattern

The trace shows repeated failed attempts:

1. **First attempt:**

   ```
   builtin_mcp_manager__listBuiltinTools(serverName: "yahoo-finance-mcp")
   → "Found 0 tools from 'yahoo-finance-mcp' server"
   ```

2. **Second attempt (after reconnect):**

   ```
   builtin_mcp_manager__listBuiltinTools(serverName: "yahoo-finance-mcp")
   → "Found 0 tools from 'yahoo-finance-mcp' server"
   ```

3. **Third, fourth, fifth attempts:**
   Same result every time

4. **Meanwhile, `verifyServer` works:**
   ```
   builtin_mcp_manager__verifyServer(name: "yahoo-finance-mcp")
   → "Available tools: 9 (cached)"
   ```

**Agent Reasoning:** "If `verifyServer` shows 9 tools, why does `listBuiltinTools` show 0?"

The agent has **no way to know** that:

- `listBuiltinTools` only works with builtin servers
- External servers are not supported by this tool
- This is expected behavior, not a server configuration issue

### Root Cause

**Lack of explicit error handling** for an invalid but reasonable use case:

1. **Tool accepts `serverName` parameter** → implies it works with any server
2. **External servers exist in registry** → can be listed via `listServers`
3. **External servers have tools** → confirmed by `verifyServer`
4. **But `listBuiltinTools` silently fails** → returns 0 without explanation

This is a **classic API design flaw**: accepting input without validating it, then failing silently.

## Comparison: Good Error Handling Example

Let's look at `verifyServer` which handles external servers correctly:

```rust
// operations.rs:verify_server
pub async fn verify_server(args: Value) -> Result<MCPResult, String> {
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => return Ok(missing_param_error("name", ToolGroup::McpManager)),
    };

    // Get configuration
    let config = match get_server_config(name).await? {
        Some(c) => c,
        None => {
            return Err(format!(
                "Server '{}' not found in configuration. \
                 Use listServers to see registered servers or \
                 registerServer to add a new one.",
                name
            ));  // ✅ Explicit error with guidance
        }
    };

    // Attempt connection and tool listing
    // ...
}
```

**Why this is better:**

- Validates input exists in registry
- Returns explicit error if not found
- Provides actionable guidance

## Proposed Fixes

### Fix #1: Add Explicit Validation and Error Messages

```rust
// queries.rs:list_builtin_tools
pub async fn list_builtin_tools(args: Value) -> Result<MCPResult, String> {
    let server_name = args
        .get("serverName")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // ✅ NEW: If server name provided, validate it's a builtin server
    if let Some(name) = &server_name {
        // Check if it's an external server
        let external_configs = list_all_configs().await?;
        let is_external = external_configs.iter().any(|cfg| {
            cfg.name.as_ref().map(|n| n == name).unwrap_or(false)
        });

        if is_external {
            return Err(format!(
                "❌ Server '{}' is an external MCP server.\n\n\
                 This tool only lists BUILTIN LibrAgent services:\n\
                 • planning, knowledge, browser, workspace\n\
                 • content_store, assistant, playbook\n\
                 • bootstrap, ui, mcp_manager\n\n\
                 📋 To check external server tools:\n\
                 1. Use verifyServer(name: '{}') to see available tools\n\
                 2. Add to assistant: updateAssistant(mcpServerIds: ['{}'])\n\
                 3. Tools load automatically when assistant session starts\n\n\
                 💡 External servers are session-scoped and discovered at runtime.",
                name, name, name
            ));
        }

        // Check if it's an unrecognized name (typo or invalid)
        let builtin_servers = [
            "planning", "knowledge", "browser", "workspace",
            "content_store", "contentstore",
            "assistant", "assistant_manager",
            "playbook", "bootstrap", "ui", "mcp_manager",
        ];

        if !builtin_servers.contains(&name.as_str()) {
            return Err(format!(
                "❌ Unknown builtin server: '{}'\n\n\
                 Valid builtin servers:\n\
                 • planning, knowledge, browser, workspace\n\
                 • content_store, assistant, playbook\n\
                 • bootstrap, ui, mcp_manager\n\n\
                 💡 Did you mean one of these? Check spelling.",
                name
            ));
        }
    }

    // Get static tool definitions (now guaranteed to be valid)
    let tools = if let Some(name) = server_name.as_ref() {
        crate::mcp::server::tools::get_static_tools_for_server(name)
    } else {
        crate::mcp::server::tools::get_all_static_builtin_tools()
    };

    // ... rest of function unchanged ...
}
```

### Fix #2: Improve Tool Description

```rust
// tools.rs:list_builtin_tools_tool()
pub fn list_builtin_tools_tool() -> MCPTool {
    MCPTool {
        name: "listBuiltinTools".to_string(),
        title: Some("List Builtin Tools".to_string()),
        description: "List tool schemas from BUILTIN LibrAgent services only.

⚠️ IMPORTANT: This tool does NOT work with external MCP servers.

BUILTIN SERVICES (supported):
• planning - Goal/todo management, task tracking
• knowledge - Information storage and retrieval
• browser - Web browsing and navigation
• workspace - File system and code execution
• content_store - Content management
• assistant - Assistant configuration
• playbook - Playbook management
• bootstrap - System initialization
• ui - UI component tools
• mcp_manager - MCP server management

EXTERNAL SERVERS (not supported):
• yahoo-finance-mcp, ddg-search, etc.
• Use verifyServer to check external server tools
• Add to assistant via mcpServerIds field

USAGE:
• No parameter: List all builtin tools (88+ tools)
• serverName='workspace': Filter by specific builtin server

PAGINATION:
Results are paginated (20 tools per page) for large result sets.
"
        .to_string(),
        input_schema: object_prop(
            vec![(
                "serverName".to_string(),
                string_prop(
                    None,
                    None,
                    Some("Optional: Builtin server name to filter by. \
                          Valid values: planning, knowledge, browser, workspace, \
                          content_store, assistant, playbook, bootstrap, ui, mcp_manager. \
                          Does NOT work with external servers."),
                ),
            )],
            vec![],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}
```

### Fix #3: Add Contextual Hints

```rust
// queries.rs - Update hints based on context
let hints = if let Some(name) = &server_name {
    // Server name was provided and valid - show server-specific hints
    vec![
        format!("Showing all tools from '{}' server", name),
        "Use listBuiltinTools() without parameters to see all servers".to_string(),
    ]
} else if total_count > PAGE_SIZE {
    // No server name, many tools - pagination hints
    vec![
        format!("Available servers: {}", available_servers.join(", ")),
        "Use serverName parameter to filter (e.g., serverName='planning')".to_string(),
        format!("Showing {}/{} tools", PAGE_SIZE, total_count),
    ]
} else {
    // No server name, few tools - basic hints
    vec![
        format!("Available servers: {}", available_servers.join(", ")),
        "Use serverName parameter to filter tools by server".to_string(),
    ]
};
```

### Fix #4: Add Logging for Debugging

```rust
// tools.rs:get_static_tools_for_server
pub fn get_static_tools_for_server(server_name: &str) -> Vec<MCPTool> {
    match server_name {
        "planning" => crate::mcp::builtin::planning::PlanningServer::tools_static(),
        // ... other cases ...
        _ => {
            log::warn!(
                "get_static_tools_for_server called with unknown server name: '{}'. \
                 This may be an external server (not supported) or a typo. \
                 Valid builtin servers: planning, knowledge, browser, workspace, \
                 content_store, assistant, playbook, bootstrap, ui, mcp_manager",
                server_name
            );
            Vec::new()
        }
    }
}
```

## Expected Behavior After Fix

### Scenario 1: External Server Name

**Input:**

```json
{ "serverName": "yahoo-finance-mcp" }
```

**Output (Error):**

```
❌ Server 'yahoo-finance-mcp' is an external MCP server.

This tool only lists BUILTIN LibrAgent services:
• planning, knowledge, browser, workspace
• content_store, assistant, playbook
• bootstrap, ui, mcp_manager

📋 To check external server tools:
1. Use verifyServer(name: 'yahoo-finance-mcp') to see available tools
2. Add to assistant: updateAssistant(mcpServerIds: ['yahoo-finance-mcp'])
3. Tools load automatically when assistant session starts

💡 External servers are session-scoped and discovered at runtime.
```

### Scenario 2: Invalid/Typo Server Name

**Input:**

```json
{ "serverName": "workspce" } // typo
```

**Output (Error):**

```
❌ Unknown builtin server: 'workspce'

Valid builtin servers:
• planning, knowledge, browser, workspace
• content_store, assistant, playbook
• bootstrap, ui, mcp_manager

💡 Did you mean one of these? Check spelling.
```

### Scenario 3: Valid Builtin Server

**Input:**

```json
{ "serverName": "workspace" }
```

**Output (Success):**

```
Found 15 tools from 'workspace' server:

• readFile - Read contents of a file (params: 1)
• writeFile - Write content to a file (params: 2)
• listDirectory - List directory contents (params: 1)
...

💡 Showing all tools from 'workspace' server
💡 Use listBuiltinTools() without parameters to see all servers
```

## Testing Plan

### Test Cases

1. **Test: External server name**
   - Input: `{"serverName": "yahoo-finance-mcp"}`
   - Expected: Error with external server guidance

2. **Test: Invalid server name (typo)**
   - Input: `{"serverName": "planing"}`
   - Expected: Error with valid server list

3. **Test: Invalid server name (unknown)**
   - Input: `{"serverName": "foobar"}`
   - Expected: Error with valid server list

4. **Test: Valid builtin server**
   - Input: `{"serverName": "workspace"}`
   - Expected: Success with tools list

5. **Test: No server name (list all)**
   - Input: `{}`
   - Expected: Success with all 88+ tools

6. **Test: Case sensitivity**
   - Input: `{"serverName": "WORKSPACE"}`
   - Expected: Error (case-sensitive match)

### Integration Test

Reproduce the exact agent trace scenario:

```rust
#[tokio::test]
async fn test_list_builtin_tools_with_external_server() {
    // Setup: Register yahoo-finance-mcp as external server
    // ...

    // Test: Call listBuiltinTools with external server name
    let result = list_builtin_tools(json!({"serverName": "yahoo-finance-mcp"})).await;

    // Assert: Should return error, not empty array
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("external MCP server"));
    assert!(result.unwrap_err().contains("verifyServer"));
}
```

## Impact Assessment

### Before Fix (Current Behavior)

- ❌ Silent failure confuses agents
- ❌ Agents waste tokens repeating failed calls
- ❌ Contradiction with `verifyServer` results
- ❌ No guidance on correct approach
- ❌ Poor user experience

### After Fix

- ✅ Explicit error messages guide users
- ✅ Clear distinction between builtin vs external
- ✅ Actionable suggestions (use `verifyServer`)
- ✅ Agents learn from first error, don't repeat
- ✅ Consistent with other tools' error handling
- ✅ Better debugging via logging

## Migration Considerations

### Breaking Changes

**Yes - Error instead of empty array**

Previous behavior:

```rust
// Returns: { "tools": [], "total": 0 }
```

New behavior:

```rust
// Returns: Err("Server 'yahoo-finance-mcp' is an external MCP server...")
```

**Impact:** Low

- Only affects code that explicitly handles empty tool arrays for external servers
- This is likely a bug/workaround, not intentional behavior
- Proper error handling is more robust

### Backward Compatibility

**Safe for most users:**

1. Valid use cases (builtin servers) still work identically
2. Invalid use cases (external servers) now fail fast with guidance
3. No API signature changes
4. Frontend/client code should already handle errors

## Naming Improvement Recommendation

### Problem: Semantic Ambiguity

The current naming creates confusion:

- **"Builtin"** vs **"External"** - Not immediately obvious to AI agents
- **"listServers"** - Unclear that it only lists external servers, not internal services

### Proposed Naming Convention

```
CURRENT                    →  IMPROVED
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
listBuiltinTools           →  listInternalTools
listServers                →  listExternalServers
verifyServer               →  verifyExternalServer
registerServer             →  registerExternalServer
updateServer               →  updateExternalServer
deleteServer               →  deleteExternalServer
```

**Benefits:**

- ✅ **Internal** vs **External** is semantically clearer
- ✅ Reduces ambiguity for AI agents parsing tool descriptions
- ✅ Makes scope immediately obvious from tool name
- ✅ Consistent namespace convention across all MCP tools

### Implementation Strategy: Deprecation Path

**Phase 1: Add New Names with Aliases (v0.4.0)**

- Introduce `listInternalTools` and `listExternalServers`
- Keep old names working with deprecation warnings
- Update documentation to recommend new names

**Phase 2: Deprecation Warnings (v0.5.0)**

- Add explicit warnings in tool descriptions
- Update all examples and documentation
- Log deprecation warnings when old names used

**Phase 3: Remove Old Names (v0.6.0)**

- Remove `listBuiltinTools` and `listServers` aliases
- Breaking change, documented in migration guide

### Updated Tool Descriptions

```rust
// NEW: listInternalTools
pub fn list_internal_tools_tool() -> MCPTool {
    MCPTool {
        name: "listInternalTools".to_string(),
        title: Some("List Internal LibrAgent Tools".to_string()),
        description: "List tool schemas from INTERNAL LibrAgent services.

🏠 INTERNAL SERVICES (this tool):
• planning, knowledge, browser, workspace
• content_store, assistant, playbook
• bootstrap, ui, mcp_manager

🌐 EXTERNAL SERVERS (use listExternalServers):
• User-registered MCP servers (yahoo-finance-mcp, ddg-search, etc.)

USAGE:
• No parameter: List all internal tools (88+ tools)
• serverName='workspace': Filter by specific internal service
".to_string(),
        // ... rest
    }
}

// DEPRECATED: listBuiltinTools (alias to listInternalTools)
pub fn list_builtin_tools_tool() -> MCPTool {
    let mut tool = list_internal_tools_tool();
    tool.name = "listBuiltinTools".to_string();
    tool.description = format!(
        "⚠️ DEPRECATED: Use 'listInternalTools' instead.\n\
         This tool will be removed in v0.6.0.\n\n{}",
        tool.description
    );
    tool
}
```

## Conclusion

The `listBuiltinTools` tool has **5 interconnected issues**:

1. ❌ Silent failure with invalid input
2. ❌ Misleading tool description
3. ❌ Non-contextual hints
4. ❌ Missing validation
5. ❌ Ambiguous naming (builtin vs external)

These issues create a **poor developer experience** and cause AI agents to waste resources on repeated failed attempts.

**Recommended Actions:**

**Immediate (v0.4.0):**

1. Implement Fix #1: Add explicit validation and error messages
2. Implement Fix #2: Improve tool descriptions
3. Implement Fix #3: Add contextual hints
4. Implement Fix #4: Add logging for debugging
5. Introduce new tool names (`listInternalTools`, `listExternalServers`) with deprecation path

**Future (v0.5.0-v0.6.0):** 6. Add deprecation warnings for old tool names 7. Complete migration to new naming convention

**Priority:** High - Affects core agent functionality and developer experience.
