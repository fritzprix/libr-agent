# Refactoring Plan: Browser Tool Session Internalization & Best Practices

**Date:** 2026-01-14 15:30  
**Priority:** High  
**Scope:** Browser builtin tool architecture refactoring

## 1. Purpose

Refactor the browser tool to follow the same session management pattern as other builtin tools (Planning, Knowledge, Playbook), where the service proxy manages session state internally rather than requiring agents to pass session IDs in every tool call.

## 2. Current State / Problems

### 2.1 Architectural Inconsistency

**Problem:** Browser tool requires `sessionId` parameter in all tool calls, unlike other builtin tools.

**Evidence:**

```rust
// navigation.rs:14-16 - Current implementation
let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
    Some(id) => id,
    None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
};
```

**Comparison with Planning tool:**

```rust
// planning/todos.rs:15-18 - Correct pattern
pub async fn add_todo(
    db: &DatabaseConnection,
    session_id: &str,  // ✅ From server instance, not args
    args: Value,
) -> Result<MCPResult, String>
```

### 2.2 Underutilized Browser Session State

**Problem:** `BrowserServer.browser_session_id` field exists but is only used for service context reporting, not for actual tool routing.

```rust
// mod.rs:41 - Field exists but underused
pub(crate) browser_session_id: Arc<RwLock<Option<String>>>,
```

### 2.3 Agent Complexity

**Problem:** Agents must track browser session IDs across multiple tool calls:

- `createSession()` returns ID
- Agent stores ID in memory/context
- Agent passes ID to `navigateToUrl(sessionId, url)`
- Agent passes ID to `extractWebContent(sessionId)`
- Risk of hallucinating or losing session ID

### 2.4 AI-Incompatible Language

**Problem:** Tool descriptions use human-centric language:

- "DO NOT use session IDs from previous attempts" (implies memory vs context)
- "Use the exact session ID from createSession response" (verbose)
- All tools require extensive session ID validation instructions

### 2.5 Missing Proactive Validation

**Problem:** URL validation is incomplete:

- No URL length limits (DoS risk)
- No `file://` protocol blocking
- No encoding validation

## 3. Related Code Structure (Bird's Eye View)

### Browser Module Structure

```
browser/
├── mod.rs              # BrowserServer struct, tool definitions, trait impl
├── session.rs          # createSession, closeSession
├── navigation.rs       # navigateToUrl, navigateBack, navigateForward, etc.
├── interaction.rs      # clickElement, inputText, scrollPage, listInteractable
└── content.rs          # extractWebContent, readWebContent
```

### Key Components

- **BrowserServer** (`mod.rs:35-45`): Holds `agent_session_id` and `browser_session_id`
- **Service Proxy** (`service_proxy.rs:17-32`): Creates isolated BrowserServer per agent
- **Service Proxy Manager** (`service_proxy_manager.rs:193-245`): Manages proxy lifecycle

### Data Flow (Current)

```
Agent → call_tool("navigateToUrl", {sessionId: "abc", url: "..."})
  → MCPServiceProxy.call_tool()
    → BrowserServer.call_tool()
      → navigation::navigate_to_url()
        → Extract sessionId from args ❌
        → Call InteractiveBrowserServer
```

### Data Flow (Target)

```
Agent → call_tool("navigateToUrl", {url: "..."})
  → MCPServiceProxy.call_tool()
    → BrowserServer.call_tool()
      → navigation::navigate_to_url()
        → Read sessionId from server.browser_session_id ✅
        → Call InteractiveBrowserServer
```

## 4. Target State / Resolution Criteria

### 4.1 Architecture Goals

- ✅ **Session Management**: Browser session ID stored in `BrowserServer` instance, not in tool parameters
- ✅ **Consistency**: All browser tools follow same pattern as Planning/Knowledge tools (no session_id in args)
- ✅ **Lifecycle**: Browser session created/destroyed automatically with agent session
- ✅ **Isolation**: Service proxy ensures each agent gets isolated browser state

### 4.2 Tool Interface Goals

**Before:**

```rust
navigateToUrl(sessionId: string, url: string) → result
```

**After:**

```rust
navigateToUrl(url: string) → result
```

### 4.3 Success Criteria

1. **No sessionId Parameters**: All browser tools remove `sessionId` from input schema
2. **Internal State Management**: `browser_session_id` used for routing, not just context
3. **Clear Error Messages**: "No active browser session. Call createSession first."
4. **AI-Compatible Language**: Tool descriptions use "Extract", "Use", "Reference" instead of "Copy", "Remember"
5. **Proactive Validation**: URL length, protocol validation before service calls
6. **Consistent Patterns**: Browser tools match Planning tool implementation style

## 5. Code to be Modified

### 5.1 `mod.rs`: Tool Schema Removal

**Remove `sessionId` parameter from ALL tools:**

```rust
// BEFORE (navigateToUrl)
input_schema: json!({
    "type": "object",
    "properties": {
        "sessionId": { "type": "string", "description": "..." },  // ❌ Remove
        "url": { "type": "string", "description": "..." }
    },
    "required": ["sessionId", "url"]  // ❌ Remove sessionId
})

// AFTER
input_schema: json!({
    "type": "object",
    "properties": {
        "url": { "type": "string", "description": "..." }
    },
    "required": ["url"]  // ✅ Only url
})
```

**Tools affected:** `navigateToUrl`, `navigateBack`, `navigateForward`, `getCurrentUrl`, `getPageTitle`, `extractWebContent`, `clickElement`, `inputText`, `scrollPage`, `listInteractable`, `readWebContent`

### 5.2 `session.rs`: createSession Refactoring

**Update to store browser session ID internally:**

```rust
// AFTER: Store session ID in server state
{
    let mut id_lock = server.browser_session_id.write().map_err(|e| e.to_string())?;
    *id_lock = Some(id.clone());
}

// Update success message (no need to expose ID to agent)
let hint = SuccessHint::new(
    format!("Browser session created. Page loaded: {}", url),  // ✅ No ID needed
    vec![
        "Use navigateToUrl to navigate to a different page".to_string(),
        "Use extractWebContent to read the current page".to_string(),
    ]
);
```

### 5.3 `navigation.rs`: Internal Session Extraction

**Replace parameter extraction with internal state lookup:**

```rust
// BEFORE
let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
    Some(id) => id,
    None => return Ok(missing_param_error("sessionId", ToolGroup::Browser)),
};

// AFTER
let browser_session_id = {
    let guard = server.browser_session_id.read().map_err(|e| e.to_string())?;
    guard.clone()
};

let browser_session_id = browser_session_id.ok_or_else(|| {
    "No active browser session. Call createSession first.".to_string()
})?;
```

**Add proactive URL validation:**

```rust
// Proactive URL validation
const MAX_URL_LENGTH: usize = 2048;

if url.len() > MAX_URL_LENGTH {
    return Ok(invalid_input_error(
        &format!("URL exceeds maximum length of {} characters", MAX_URL_LENGTH),
        ToolGroup::Browser,
    ));
}

if url.starts_with("file://") {
    return Ok(invalid_input_error(
        "Local file URLs are not supported. Use http:// or https:// URLs only",
        ToolGroup::Browser,
    ));
}
```

### 5.4 `interaction.rs`, `content.rs`: Same Pattern

Apply same session extraction pattern to all tool functions:

- `click_element`
- `input_text`
- `scroll_page`
- `list_interactable`
- `extract_web_content`
- `read_web_content`

## 6. Reusable Related Code

### 6.1 Planning Tool Reference (Session Management Pattern)

**File:** `src-tauri/src/mcp/builtin/planning/todos.rs`

```rust
// Function signature pattern to follow
pub async fn add_todo(
    db: &DatabaseConnection,
    session_id: &str,  // ✅ From server, not args
    args: Value,
) -> Result<MCPResult, String> {
    let description = args.get("description").and_then(|v| v.as_str());
    // No session_id extraction from args
}
```

**File:** `src-tauri/src/mcp/builtin/planning/mod.rs`

```rust
// Server structure pattern
pub struct PlanningServer {
    session_id: String,  // ✅ Stored in server
    db: Arc<DatabaseConnection>,
}

// call_tool routing pattern
async fn call_tool(&self, tool_name: &str, args: Value, _session_id: Option<String>)
    -> Result<MCPResult, String>
{
    match tool_name {
        "createTodo" => todos::add_todo(&self.db, &self.session_id, args).await,
        //                                        ^^^^^^^^^^^^^^^^
        //                                        Pass from server instance
    }
}
```

### 6.2 Error Guidance Module

**File:** `src-tauri/src/mcp/builtin/error_guidance.rs`

Use existing error functions:

- `missing_param_error(param, tool_group)`
- `invalid_input_error(message, tool_group)`
- `operation_failed_error(operation, error, guidance, tool_group)`
- `SuccessHint::new(message, suggestions)`

## 7. Test Code Guide

### 7.1 Unit Tests

Add to `src-tauri/src/mcp/builtin/browser/mod.rs` (or separate test file):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_session_id_in_tool_schemas() {
        let server = BrowserServer::new(/* ... */);
        let tools = server.tools();

        for tool in tools {
            let schema = tool.input_schema;
            let props = schema["properties"].as_object().unwrap();

            // Verify no sessionId parameter
            assert!(
                !props.contains_key("sessionId"),
                "Tool '{}' should not have sessionId parameter",
                tool.name
            );
        }
    }

    #[tokio::test]
    async fn test_navigate_without_session_returns_error() {
        let server = BrowserServer::new(/* ... */);

        // Browser session not created yet
        let result = navigation::navigate_to_url(
            &server,
            json!({"url": "https://example.com"})
        ).await;

        assert!(result.is_ok());
        let mcp_result = result.unwrap();
        assert_eq!(mcp_result.is_error, Some(true));
        // Check error message contains "No active browser session"
    }
}
```

### 7.2 Integration Tests

Add to `service_proxy_manager.rs` tests:

```rust
#[tokio::test]
async fn test_browser_session_isolation() {
    let manager = create_test_manager().await;

    // Create two agents with browser tool
    let session1 = "agent-1".to_string();
    let session2 = "agent-2".to_string();

    manager.create_proxy(session1.clone(), vec!["browser".to_string()], Some(app_handle)).await.unwrap();
    manager.create_proxy(session2.clone(), vec!["browser".to_string()], Some(app_handle)).await.unwrap();

    // Agent 1 creates browser session
    let result1 = manager.call_tool(&session1, "builtin_browser__createSession", json!({"url": "https://google.com"})).await.unwrap();
    assert!(result1.error.is_none());

    // Agent 2's browser should still have no session
    let result2 = manager.call_tool(&session2, "builtin_browser__navigateToUrl", json!({"url": "https://example.com"})).await.unwrap();
    // Should error: no session

    // Agent 2 creates its own session
    let result2 = manager.call_tool(&session2, "builtin_browser__createSession", json!({"url": "https://example.com"})).await.unwrap();
    assert!(result2.error.is_none());

    // Both agents can navigate independently
    // (each uses their own isolated browser session)
}
```

## 8. Implementation Phases

### Phase 1: Internal State Management (Core Changes)

1. Update `session::create_session` to store browser session ID
2. Update `session::close_session` to clear internal state
3. Update all tool functions in `navigation.rs` to extract session from server
4. Update all tool functions in `interaction.rs` to extract session from server
5. Update all tool functions in `content.rs` to extract session from server

### Phase 2: Schema & Description Updates

6. Remove `sessionId` from all tool input schemas in `mod.rs`
7. Update tool descriptions to remove session ID language
8. Add AI-compatible workflow descriptions
9. Add proactive URL validation

### Phase 3: Testing & Validation

10. Add unit tests for schema validation
11. Add integration tests for session isolation
12. Test error cases (no session, invalid URLs)
13. Validate with real agent workflows

## 9. Breaking Changes & Migration

### 9.1 Breaking Changes

- **API Change:** All browser tools no longer accept `sessionId` parameter
- **Behavior Change:** Browser session is automatically managed by service proxy
- **Error Messages:** New error when calling tools without active session

### 9.2 Migration Guide (for existing agents/workflows)

**Before:**

```typescript
const session = await callTool('createSession', { url: 'https://google.com' });
const sessionId = extractSessionId(session);
await callTool('navigateToUrl', { sessionId, url: 'https://example.com' });
await callTool('extractWebContent', { sessionId });
```

**After:**

```typescript
await callTool('createSession', { url: 'https://google.com' });
await callTool('navigateToUrl', { url: 'https://example.com' });
await callTool('extractWebContent', {});
```

### 9.3 Backward Compatibility

- ❌ Not maintained (breaking change accepted)
- Reason: Internal tool, consistency with other builtin tools more important
- Impact: Low (browser tool is relatively new, limited production usage)

## 10. Clarification Q-list

- **Q1**: Should `createSession` still return the browser session ID in structured_content for debugging purposes?
  - **A**: Yes, keep in `structured_content` for UI/debugging, but remove from text content visible to AI
- **Q2**: Should we support multiple browser sessions per agent in future versions?
  - **A**: Not in v1.0. If needed, add named sessions later (`createSession(name?)`, `navigateToUrl(url, sessionName?)`)
- **Q3**: What happens if agent calls `createSession` twice?
  - **A**: Close existing session and create new one (current behavior, keep it)
- **Q4**: Should `closeSession` be automatic when agent terminates?
  - **A**: Yes, service proxy cleanup handles this (already works via Drop trait)

## 11. Success Metrics

- ✅ All browser tools have 0 `sessionId` parameters
- ✅ All browser tool functions use internal state for session lookup
- ✅ Agent workflows no longer need to track browser session IDs
- ✅ Error messages clearly guide agents to call `createSession` first
- ✅ Unit tests verify schema correctness
- ✅ Integration tests verify session isolation
- ✅ Documentation updated (tool descriptions, best practices guide)

---

**Estimated Effort:** 4-6 hours  
**Risk Level:** Medium (breaking change, but well-scoped)  
**Dependencies:** None (self-contained refactoring)
