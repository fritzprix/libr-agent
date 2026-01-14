# Best Practices for Built-in Tool Design

This document outlines the common design patterns, principles, and best practices that apply to all built-in MCP tools in LibrAgent. These practices are derived from successful implementations like the Browser, Planning, and Workspace tools.

---

## Table of Contents

1. [Architectural Principles](#1-architectural-principles)
2. [Module Structure](#2-module-structure)
3. [AI-Compatible Tool Descriptions](#3-ai-compatible-tool-descriptions)
4. [Tool Response Design](#4-tool-response-design)
5. [Error Handling System](#5-error-handling-system)
6. [Service Context Pattern](#6-service-context-pattern)
7. [Performance Optimization](#7-performance-optimization)
8. [Tool Chaining & Guidance](#8-tool-chaining--guidance)
9. [Testing & Validation](#9-testing--validation)

---

## 1. Architectural Principles

### 1.1 Trait-Based Interface

All built-in tools implement the `BuiltinMCPServer` trait:

```rust
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn tools(&self) -> Vec<MCPTool>;
    async fn call_tool(&self, tool_name: &str, args: Value, session_id: Option<String>) -> Result<MCPResult, String>;
    async fn get_service_context(&self, options: Option<&Value>) -> ServiceContext;
}
```

**Why:** Ensures consistent interface across all tools and enables polymorphic handling in the registry.

### 1.2 Session Isolation

Each agent session gets its own isolated server instance:

```rust
pub struct BrowserServer {
    pub(crate) agent_session_id: String,  // Unique per agent
    pub(crate) browser_session_id: Arc<RwLock<Option<String>>>, // Isolated state
    // ...
}
```

**Benefits:**

- Prevents state leakage between agents
- Enables parallel agent execution
- Simplifies cleanup on session termination

### 1.3 State Management

Use `Arc<RwLock<>>` for shared mutable state:

```rust
use std::sync::{Arc, RwLock};

pub struct MyServer {
    state: Arc<RwLock<ServerState>>,
}
```

**Pattern:**

- Read locks: `self.state.read().map_err(|e| e.to_string())?`
- Write locks: `self.state.write().map_err(|e| e.to_string())?`
- Always handle lock poisoning errors

---

## 2. Module Structure

### 2.1 Feature-Based Organization

Organize tools by feature domains:

```text
tool_name/
├── mod.rs           # Main server struct, tool definitions, routing
├── session.rs       # Session lifecycle (create, close)
├── operations.rs    # Core operations (CRUD, etc.)
├── queries.rs       # Read-only operations
└── helpers.rs       # Internal utilities
```

**Example (Browser):**

```text
browser/
├── mod.rs           # BrowserServer + tool definitions
├── session.rs       # createSession, closeSession
├── navigation.rs    # navigateToUrl, navigateBack, getCurrentUrl
├── interaction.rs   # clickElement, inputText, scrollPage
└── content.rs       # extractWebContent, readWebContent
```

### 2.2 Tool Routing Pattern

Use match statements in `call_tool`:

```rust
async fn call_tool(&self, tool_name: &str, args: Value, _session_id: Option<String>)
    -> Result<MCPResult, String>
{
    match tool_name {
        "createSession" => session::create_session(self, args).await,
        "operation1" => operations::operation1(self, args).await,
        "query1" => queries::query1(self, args).await,
        _ => Err(format!("Unknown tool: {}", tool_name)),
    }
}
```

**Benefits:**

- Clear separation of concerns
- Easy to add/remove tools
- Compiler catches missing implementations

---

## 3. AI-Compatible Tool Descriptions

### 3.1 Understanding AI Agent Capabilities

**CRITICAL:** Tool descriptions and parameter docs are read by AI agents, not humans. AI agents:

- ✅ **CAN:** Read and reference text from previous tool responses
- ✅ **CAN:** Extract values from structured data in context
- ✅ **CAN:** Match text patterns exactly as shown
- ❌ **CANNOT:** "Copy" text (no clipboard/copy-paste concept)
- ❌ **CANNOT:** Distinguish "from memory" vs "from output" (everything is context data)
- ❌ **CANNOT:** Execute human physical actions

### 3.2 Language Do's and Don'ts

#### ❌ **Avoid Human-Centric Verbs:**

```rust
// ❌ WRONG: Human physical actions
"COPY the exact text from readFile output"
"Copy text directly from readFile output - do NOT use from memory"
"Paste the content into this parameter"
"Highlight the section you want to replace"

// ❌ WRONG: Ambiguous cognitive instructions
"Remember the previous value"
"Don't use from memory" (everything AI uses IS memory)
"Recall the session ID"
```

#### ✅ **Use AI-Compatible Verbs:**

```rust
// ✅ CORRECT: Data operations AI can perform
"Extract the exact text from readFile response"
"Use text exactly as shown in readFile response"
"Reference the value returned by previousTool"
"Match the text precisely as displayed"

// ✅ CORRECT: Clear data flow instructions
"Use the session_id returned by createSession"
"Ensure text matches current file content from readFile"
"Include the exact value from the response"
```

### 3.3 Parameter Description Template

```rust
item_props.insert(
    "paramName".to_string(),
    string_prop(
        None,
        None,
        Some("⚠️ CRITICAL: [What this parameter is]

MANDATORY WORKFLOW:
1. Call [prerequisiteTool] FIRST to get current data
2. Extract the exact [dataType] from [prerequisiteTool] response
3. Include [context requirements] for uniqueness
4. Use the extracted [dataType] as this parameter

❌ NEVER use [dataType] reconstructed from previous attempts
✅ ALWAYS use [dataType] exactly as returned by [prerequisiteTool]"),
    ),
);
```

### 3.4 Tool Description Template

```rust
MCPTool {
    name: "toolName".to_string(),
    description: "[Brief description of what tool does]

⚠️ CRITICAL WORKFLOW (MUST FOLLOW):
1. ALWAYS call [prerequisiteTool] FIRST
2. Extract the exact [data] from [prerequisiteTool] response into [paramName] parameter
3. Verify the extracted data includes [requirements]
4. Then call toolName with the extracted [paramName]

❌ NEVER use [paramName] reconstructed from previous attempts or assumptions
✅ ALWAYS use data exactly as returned by [prerequisiteTool] to ensure exact match
⚠️ If operation fails, DO NOT retry with the same [paramName] - call [prerequisiteTool] again first".to_string(),
    // ...
}
```

### 3.5 Real-World Example: replaceStringInFile

**Before (Human-Centric):**

```rust
Some("⚠️ CRITICAL: Exact text content to find and replace.

MANDATORY WORKFLOW:
1. Call readFile(path) FIRST to get current content
2. COPY the exact text from readFile output (including all whitespace)
3. Include surrounding context (3-5 lines) for uniqueness
4. Use the copied text as this parameter

❌ NEVER use text from memory or previous attempts
✅ ALWAYS copy directly from readFile output")
```

**After (AI-Compatible):**

```rust
Some("⚠️ CRITICAL: Exact text content to find and replace.

MANDATORY WORKFLOW:
1. Call readFile(path) FIRST to get current content
2. Extract the exact text from readFile response (including all whitespace)
3. Include surrounding context (3-5 lines) for uniqueness
4. Use the extracted text as this parameter

❌ NEVER use text reconstructed from previous attempts
✅ ALWAYS use text exactly as shown in readFile response")
```

**Key Changes:**

- `"COPY"` → `"Extract"` (data operation vs physical action)
- `"from readFile output"` → `"from readFile response"` (consistent terminology)
- `"from memory"` → `"reconstructed from previous attempts"` (specific and meaningful to AI)
- `"copy directly"` → `"use exactly as shown"` (achievable instruction)

### 3.6 Error Message Language

Apply same principles to error guidance:

```rust
// ❌ WRONG: Human instructions
ErrorGuidance::with_guidance(
    ErrorCategory::InvalidInput,
    "Pattern not found",
    vec![
        "Copy text directly from readFile output - do NOT use from memory".to_string(),
    ],
    ToolGroup::Workspace,
)

// ✅ CORRECT: AI-compatible instructions
ErrorGuidance::with_guidance(
    ErrorCategory::InvalidInput,
    "Pattern not found",
    vec![
        "Use text exactly as shown in readFile response - ensure it matches current file content".to_string(),
    ],
    ToolGroup::Workspace,
)
```

### 3.7 Validation Checklist

Before finalizing tool descriptions:

- [ ] No "COPY", "copy", "paste" verbs used
- [ ] No "from memory" vs "from output" distinctions
- [ ] No human physical action verbs ("highlight", "select", "click in description text")
- [ ] Use "extract", "use", "reference", "match" instead
- [ ] Workflow steps are data operations AI can perform
- [ ] Success/error messages follow same language rules
- [ ] Instructions are specific and actionable for AI processing model

---

## 4. Tool Response Design

### 4.1 MCPResult Structure

**CRITICAL UNDERSTANDING:**

```rust
pub struct MCPResult {
    pub content: Option<Vec<MCPContent>>,           // ✅ AI sees this
    pub structured_content: Option<serde_json::Value>, // ❌ AI NEVER sees this
    pub is_error: Option<bool>,
}
```

**Data Flow:**

1. `content` → System prompt → AI reads it
2. `structured_content` → UI components only (debugging, rendering)

### 4.2 Success Response Pattern

#### **Use SuccessHint for all successful operations:**

```rust
use crate::mcp::builtin::error_guidance::SuccessHint;

let hint = SuccessHint::new(
    "Operation successful: details here",  // What happened
    vec![
        "Use nextTool1 to continue".to_string(),
        "Use nextTool2 as alternative".to_string(),
    ] // Suggested next actions
);
Ok(hint.to_mcp_result())
```

**Result Format:**

```text
✓ Operation successful: details here

💡 Next: Use nextTool1 to continue or Use nextTool2 as alternative
```

#### **Include IDs and Identifiers in Text:**

```rust
// ❌ WRONG: AI can't see the ID
let hint = SuccessHint::new(
    "Session created successfully",
    vec![/* suggestions */]
);
Ok(hint.to_mcp_result_with_data(Some(json!({"session_id": "abc123"}))))

// ✅ CORRECT: ID visible in text
let hint = SuccessHint::new(
    format!("Session created: {}", session_id),  // ID in text!
    vec!["Use navigateToUrl(sessionId) to load a page".to_string()]
);
Ok(hint.to_mcp_result_with_data(Some(json!({"session_id": session_id}))))
```

### 4.3 List Response Pattern

When returning lists, format them clearly in text:

```rust
// Format list items with indices/identifiers
let items_text = items.iter()
    .map(|item| format!("• {} [{}]: {}", item.id, item.status, item.name))
    .collect::<Vec<_>>()
    .join("\n");

let hint = SuccessHint::new(
    format!("Found {} items:\n\n{}", items.len(), items_text),
    vec!["Use processItem(itemId) to work with an item".to_string()]
);
```

**Result:**

```text
✓ Found 3 items:

• abc123 [active]: Item One
• def456 [pending]: Item Two
• ghi789 [complete]: Item Three

💡 Next: Use processItem(itemId) to work with an item
```

### 4.4 Pagination Pattern

For large content, use consistent pagination:

```rust
// First page response
let response_text = if auto_merged {
    format!("✓ Content extracted and auto-merged\n\n{}", full_content)
} else {
    format!("[Page 1/{}]\n\n{}\n\n--- End of Page 1 ---\nUse readContent(page) to read pages 2-{}.",
        total_pages, first_page, total_pages)
};

// Individual page response
format!("[Page {}/{}]\n\n{}", current_page, total_pages, page_content)
```

---

## 5. Error Handling System

### 5.1 Four-Layer Error Handling

#### **Layer 1: Proactive Validation**

Validate inputs BEFORE operations:

```rust
// ❌ DON'T: Let operation fail
let result = service.process(selector).await?;

// ✅ DO: Validate first
if selector.trim().is_empty() {
    return Ok(invalid_input_error(
        "Selector cannot be empty",
        ToolGroup::YourTool,
    ));
}

if !url.starts_with("http://") && !url.starts_with("https://") {
    return Ok(invalid_input_error(
        "URL must start with http:// or https://",
        ToolGroup::YourTool,
    ));
}

let result = service.process(selector).await?;
```

#### **Layer 2: Use Standard Error Functions**

```rust
use crate::mcp::builtin::error_guidance::{
    missing_param_error,
    not_found_error,
    duplicate_error,
    invalid_input_error,
    permission_denied_error,
    operation_failed_error,
    ToolGroup,
};

// Missing parameter
let session_id = match args.get("sessionId").and_then(|v| v.as_str()) {
    Some(id) => id,
    None => return Ok(missing_param_error("sessionId", ToolGroup::YourTool)),
};

// Resource not found
if !resource_exists {
    return Ok(not_found_error("Session", &session_id, ToolGroup::YourTool));
}

// Duplicate resource
if already_exists {
    return Ok(duplicate_error("Todo", &title, ToolGroup::Planning));
}

// Invalid input
if invalid_format {
    return Ok(invalid_input_error("Invalid URL format", ToolGroup::Browser));
}
```

#### **Layer 3: Context-Specific Error Handling**

Parse operation results and provide targeted guidance:

```rust
let result = match service.click_element(selector).await {
    Ok(res) => {
        if res.contains("Element not found") {
            return Ok(operation_failed_error(
                "Click element",
                &format!("Element '{}' not found", selector),
                vec![
                    "Verify the selector is correct CSS syntax".to_string(),
                    "Use scrollPage to reveal lazy-loaded elements".to_string(),
                    "Use listInteractable to find valid selectors".to_string(),
                ],
                ToolGroup::Browser,
            ));
        }
        if res.contains("Element not visible") {
            return Ok(operation_failed_error(
                "Click element",
                &format!("Element '{}' is hidden", selector),
                vec![
                    "Use extractWebContent to find parent containers".to_string(),
                    "Use scrollPage to trigger visibility".to_string(),
                ],
                ToolGroup::Browser,
            ));
        }
        res
    }
    Err(e) => {
        return Ok(operation_failed_error(
            "Click element",
            &e,
            vec![/* default guidance */],
            ToolGroup::Browser,
        ))
    }
};
```

#### **Layer 4: Global Error Handler (Optional)**

For common error patterns, create a shared handler:

```rust
pub(crate) fn handle_tool_error(
    operation: &str,
    error: String,
    default_guidance: Vec<&str>,
) -> MCPResult {
    let error_lower = error.to_lowercase();

    let guidance = if error_lower.contains("timeout") {
        vec!["Operation timed out. Try again with simpler parameters."]
    } else if error_lower.contains("permission") {
        vec!["Permission denied. Check access rights."]
    } else {
        default_guidance
    };

    let guidance_strings: Vec<String> = guidance.iter().map(|s| s.to_string()).collect();
    operation_failed_error(operation, &error, guidance_strings, ToolGroup::YourTool)
}
```

### 5.2 Error Categories

Use appropriate error categories:

```rust
pub enum ErrorCategory {
    // Input validation errors (user-fixable)
    MissingRequiredParam,    // Missing required parameter
    InvalidInput,            // Invalid parameter value
    InvalidFormat,           // Wrong format (URL, JSON, etc.)

    // State/resource errors (context-dependent)
    ResourceNotFound,        // Resource doesn't exist
    DuplicateResource,       // Resource already exists
    InvalidState,            // Operation not valid in current state
    NestingTooDeep,         // Exceeded depth limit

    // Operation failures (may be transient)
    OperationFailed,         // Operation failed for external reasons
    Timeout,                 // Operation timed out
    NetworkError,            // Network connectivity issue

    // System errors (escalation needed)
    InternalError,           // Internal system error
    DatabaseError,           // Database operation failed
    PermissionDenied,        // Access denied
}
```

### 5.3 Tool Group Isolation

**CRITICAL:** Only suggest tools from the same group:

```rust
pub enum ToolGroup {
    Browser,      // Browser tools only
    Planning,     // Planning tools only
    Workspace,    // Workspace tools only
    Assistant,    // Assistant tools only
    ContentStore, // Content store tools only
    Knowledge,    // Knowledge tools only
    // ...
}
```

**Example:**

```rust
// ❌ WRONG: Browser error suggests Planning tool
return Ok(operation_failed_error(
    "Navigate",
    "Page not found",
    vec!["Use createPlan to organize tasks".to_string()],  // Wrong!
    ToolGroup::Browser,
));

// ✅ CORRECT: Browser error suggests Browser tools
return Ok(operation_failed_error(
    "Navigate",
    "Page not found",
    vec![
        "Use navigateBack to return to previous page".to_string(),
        "Use createSession to start fresh".to_string(),
    ],
    ToolGroup::Browser,
));
```

### 5.4 Error Response Format

All errors follow this format:

```text
✗ Operation failed: specific reason

💡 Next Steps:
1. First actionable step
2. Second actionable step
3. Third actionable step
```

**Principles:**

- ✗ symbol for errors, ✓ for success
- 💡 symbol for guidance
- 2-3 actionable recovery steps
- No internal state exposure
- No stack traces or debug info

---

## 6. Service Context Pattern

### 6.1 Purpose

Service context injects current tool state into the system prompt:

```rust
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    ServiceContext {
        context_prompt: "## Tool Name\n\nCurrent state here",  // ✅ AI sees this
        structured_state: Some(json!({"key": "value"})),       // ❌ AI doesn't see this
    }
}
```

**When to Use:**

- Tool has persistent state (sessions, active tasks, etc.)
- AI needs to know current context to make decisions
- Avoid redundant parameter passing

**When NOT to Use:**

- Stateless tools (one-shot operations)
- State is always passed as tool parameters

### 6.2 Implementation Pattern

```rust
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    // 1. Check if state exists
    let state = match self.state.read() {
        Ok(guard) => guard.clone(),
        Err(_) => {
            return ServiceContext {
                context_prompt: "## Tool Name\n\nNo active state".to_string(),
                structured_state: Some(json!({"active": false})),
            };
        }
    };

    // 2. Format state as readable text
    let context_prompt = format!(
        "## Tool Name\n\nActive Session: {}\nCurrent Status: {}",
        state.session_id,
        state.status
    );

    // 3. Include full details in structured_state (for UI)
    ServiceContext {
        context_prompt,
        structured_state: Some(json!({
            "active": true,
            "session_id": state.session_id,
            "status": state.status,
            "details": state.details,
        })),
    }
}
```

### 6.3 Caching Strategy (Optional)

For expensive operations (e.g., JS injection), implement caching:

```rust
pub struct MyServer {
    state_cache: Arc<RwLock<Option<(String, Instant)>>>, // (data, timestamp)
}

async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    const CACHE_TTL_SECS: u64 = 5;

    // Check cache first
    if let Ok(cache_guard) = self.state_cache.read() {
        if let Some((cached_data, last_update)) = cache_guard.as_ref() {
            if last_update.elapsed().as_secs() < CACHE_TTL_SECS {
                return ServiceContext {
                    context_prompt: format!("## Tool\n\n{}", cached_data),
                    structured_state: Some(json!({"cached": true})),
                };
            }
        }
    }

    // Cache miss - fetch fresh data
    let fresh_data = self.fetch_state().await?;

    // Update cache
    if let Ok(mut cache_guard) = self.state_cache.write() {
        *cache_guard = Some((fresh_data.clone(), Instant::now()));
    }

    ServiceContext {
        context_prompt: format!("## Tool\n\n{}", fresh_data),
        structured_state: Some(json!({"cached": false})),
    }
}
```

### 6.4 Cache Invalidation

Invalidate cache after state-changing operations:

```rust
impl MyServer {
    pub(crate) fn invalidate_cache(&self) {
        if let Ok(mut cache_guard) = self.state_cache.write() {
            *cache_guard = None;
        }
    }
}

// In tool implementation
pub async fn update_state(server: &MyServer, args: Value) -> Result<MCPResult, String> {
    // Perform operation
    server.service.update(&args).await?;

    // Invalidate cache
    server.invalidate_cache();

    Ok(SuccessHint::new("State updated", vec![]).to_mcp_result())
}
```

---

## 7. Performance Optimization

### 7.1 Async Runtime Rules

Never block the async runtime with CPU-intensive operations:

```rust
// ❌ WRONG: Blocking operation on async thread
let markdown = convert_html_to_markdown(&html); // CPU-intensive!

// ✅ CORRECT: Offload to blocking thread
let html_clone = html.clone();
let markdown = tokio::task::spawn_blocking(move || {
    convert_html_to_markdown(&html_clone)
})
.await
.map_err(|e| format!("Task join error: {}", e))?;
```

**When to use `spawn_blocking`:**

- CPU-intensive computations (parsing, conversion, encryption)
- Synchronous I/O (file operations, legacy libraries)
- Operations taking > 10ms on a single thread

### 7.2 Input Size Limits

Prevent resource exhaustion:

```rust
// Check size before processing
const MAX_SIZE_BYTES: usize = 10 * 1024 * 1024; // 10 MB

if input.len() > MAX_SIZE_BYTES {
    return Ok(invalid_input_error(
        &format!("Input exceeds maximum size of {} MB", MAX_SIZE_BYTES / 1024 / 1024),
        ToolGroup::YourTool,
    ));
}

// Process input
let result = process_large_data(&input)?;
```

### 7.3 Pagination

For large result sets, implement pagination:

```rust
// Store content with pagination metadata
pub struct ContentStore {
    sessions: Arc<RwLock<HashMap<String, Vec<String>>>>, // session_id -> pages
}

impl ContentStore {
    pub fn save_content(&self, session_id: &str, content: String, page_size_tokens: usize)
        -> (usize, String, Option<String>)
    {
        // Split content into pages
        let pages = self.paginate_by_tokens(&content, page_size_tokens);
        let total_pages = pages.len();
        let first_page = pages.first().cloned().unwrap_or_default();

        // Auto-merge if content is small
        let merged = if total_pages <= 2 && content.len() < 5000 {
            Some(content)
        } else {
            None
        };

        // Store all pages
        if let Ok(mut guard) = self.sessions.write() {
            guard.insert(session_id.to_string(), pages);
        }

        (total_pages, first_page, merged)
    }

    pub fn get_page(&self, session_id: &str, page: usize) -> Option<String> {
        let guard = self.sessions.read().ok()?;
        let pages = guard.get(session_id)?;
        pages.get(page - 1).cloned()
    }
}
```

### 7.4 Resource Cleanup

Implement cleanup for long-lived resources:

```rust
impl MyServer {
    pub async fn cleanup_session(&self, session_id: &str) -> Result<(), String> {
        // Close active connections
        self.close_connections(session_id).await?;

        // Clear cached data
        self.clear_cache(session_id)?;

        // Remove from state
        if let Ok(mut guard) = self.sessions.write() {
            guard.remove(session_id);
        }

        Ok(())
    }
}
```

---

## 8. Tool Chaining & Guidance

### 8.1 Success Hints

Always provide next-step suggestions:

```rust
// ❌ WRONG: No guidance
Ok(MCPResult::success("Session created"))

// ✅ CORRECT: Suggest next tools
let hint = SuccessHint::new(
    format!("Session created: {}", session_id),
    vec![
        "Use navigateTo(sessionId, url) to load a page".to_string(),
        "Use listTools(sessionId) to see available actions".to_string(),
    ]
);
Ok(hint.to_mcp_result())
```

### 8.2 Tool Group Context

Suggestions must respect tool groups:

```rust
// Browser tool success
SuccessHint::new(
    "Page loaded successfully",
    vec![
        "Use extractContent to see page content".to_string(),      // ✅ Browser tool
        "Use listInteractable to see clickable elements".to_string(), // ✅ Browser tool
        // ❌ Never: "Use createTodo to track progress" (Planning tool)
    ]
)

// Planning tool success
SuccessHint::new(
    "Todo created",
    vec![
        "Use getCurrentState to see all todos".to_string(),  // ✅ Planning tool
        "Use checkTodo to mark as done".to_string(),        // ✅ Planning tool
        // ❌ Never: "Use navigateBack to return" (Browser tool)
    ]
)
```

### 8.3 Conditional Guidance

Provide context-specific suggestions:

```rust
let suggestions = if total_pages > 1 {
    vec![format!("Use readPage({}, 2) to read next page", session_id)]
} else if is_final_page {
    vec![
        "All content read. Process the information gathered".to_string(),
        "Use summarizeContent to create a summary".to_string(),
    ]
} else {
    vec!["Use extractContent to capture more data".to_string()]
};

let hint = SuccessHint::new(result_message, suggestions);
Ok(hint.to_mcp_result())
```

### 8.4 Error Recovery Guidance

Errors should guide toward recovery:

```rust
// ❌ WRONG: No recovery path
return Ok(MCPResult::error("Session not found"));

// ✅ CORRECT: Actionable recovery steps
return Ok(operation_failed_error(
    "Get session",
    "Session 'abc123' not found or expired",
    vec![
        "Use createSession to start a new session".to_string(),
        "Use listSessions to see active sessions".to_string(),
        "Verify the session ID is correct".to_string(),
    ],
    ToolGroup::YourTool,
));
```

---

## 9. Testing & Validation

### 9.1 Unit Tests

Test error guidance formatting:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_has_visual_markers() {
        let error = ErrorGuidance::new(
            ErrorCategory::ResourceNotFound,
            "Session not found",
            ToolGroup::YourTool,
        );

        let result = error.to_mcp_result();
        assert_eq!(result.is_error, Some(true));

        if let Some(content) = result.content {
            if let Some(MCPContent::Text { text }) = content.first() {
                assert!(text.contains("✗"));
                assert!(text.contains("💡 Next Steps:"));
            }
        }
    }

    #[test]
    fn test_tool_group_isolation() {
        let error = ErrorGuidance::new(
            ErrorCategory::ResourceNotFound,
            "Item not found",
            ToolGroup::Planning,
        );

        // Should suggest Planning tools only
        assert!(error.guidance.iter().any(|g| g.contains("getCurrentState")));

        // Should NOT suggest Browser tools
        assert!(!error.guidance.iter().any(|g| g.contains("navigateToUrl")));
    }
}
```

### 9.2 Integration Tests

Test tool execution flow:

```rust
#[tokio::test]
async fn test_create_and_query_flow() {
    let server = MyServer::new(/* ... */);

    // Create resource
    let result = server.call_tool(
        "createResource",
        json!({"name": "test"}),
        Some("session123".to_string()),
    ).await.unwrap();

    assert_eq!(result.is_error, Some(false));

    // Query resource
    let result = server.call_tool(
        "getResource",
        json!({"name": "test"}),
        Some("session123".to_string()),
    ).await.unwrap();

    assert_eq!(result.is_error, Some(false));
}
```

### 9.3 Validation Checklist

Before deploying a new tool:

- [ ] All required parameters validated proactively
- [ ] Error messages include visual markers (✗, 💡)
- [ ] Success messages include next-step suggestions
- [ ] Tool group isolation maintained (no cross-tool suggestions)
- [ ] Tool descriptions use AI-compatible language (no "COPY", "from memory")
- [ ] IDs and identifiers visible in text responses
- [ ] Large operations use `spawn_blocking`
- [ ] Input size limits enforced
- [ ] Service context returns readable text
- [ ] Cache invalidation implemented (if caching used)
- [ ] Resource cleanup on session termination
- [ ] Unit tests cover error formatting
- [ ] Integration tests cover happy path

---

## Summary

### Key Takeaways

1. **AI Sees Text Only**: Everything important must be in `content` field, not `structured_content`
2. **AI-Compatible Language**: Use "extract", "use", "reference" instead of "copy", "paste", "from memory"
3. **Four-Layer Errors**: Proactive validation → Standard functions → Context-specific → Global handler
4. **Tool Isolation**: Only suggest tools from the same tool group
5. **Success Hints**: Always provide 2-3 actionable next steps
6. **Performance**: Use `spawn_blocking` for CPU-intensive work
7. **Service Context**: Inject state as readable text with 5-second cache TTL
8. **Visual Markers**: ✓ for success, ✗ for errors, 💡 for guidance

### Anti-Patterns to Avoid

❌ Storing critical IDs only in `structured_content`  
❌ Using human-centric verbs in tool descriptions ("COPY", "paste", "from memory")  
❌ Suggesting tools from different tool groups in errors  
❌ Blocking async runtime with CPU-intensive operations  
❌ Missing proactive input validation  
❌ Generic error messages without recovery guidance  
❌ Success messages without next-step suggestions  
❌ Exposing internal state or stack traces in errors

### Reference Implementations

- **Browser Tool**: Session management, caching, pagination (`src-tauri/src/mcp/builtin/browser/`)
- **Planning Tool**: State tracking, hierarchy validation (`src-tauri/src/mcp/builtin/planning/`)
- **Workspace Tool**: AI-compatible tool descriptions (`src-tauri/src/mcp/builtin/workspace/tools/file_tools.rs`)
- **Error Guidance**: Centralized error system (`src-tauri/src/mcp/builtin/error_guidance.rs`)

---

**Last Updated:** January 14, 2026  
**Version:** 1.1  
**Maintainers:** LibrAgent Core Team
