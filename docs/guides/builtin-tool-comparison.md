# Built-in Tool Comparison Guide: Legacy vs New Implementation

> **Target Audience**: Developers working on LibrAgent built-in tools  
> **Purpose**: Understand the evolution from legacy to new tool implementation patterns  
> **Last Updated**: December 31, 2025

## Table of Contents

1. [Overview](#overview)
2. [Architecture Comparison](#architecture-comparison)
3. [Tool Description](#tool-description)
4. [Input Validation](#input-validation)
5. [Error Handling](#error-handling)
6. [Tool Result Format](#tool-result-format)
7. [Migration Checklist](#migration-checklist)
8. [Code Examples](#code-examples)
9. [Testing Comparison](#testing-comparison)

---

## Overview

LibrAgent's built-in tools have evolved significantly to improve consistency, developer experience, and AI agent usability. This guide compares **legacy patterns** with **new standardized patterns** across four critical dimensions:

| Dimension | Legacy Approach | New Approach |
|-----------|----------------|--------------|
| **Tool Description** | Inconsistent schema structure | Standardized JSONSchema with examples |
| **Input Validation** | Ad-hoc string checks | Schema-driven with typed validation |
| **Error Handling** | Raw error strings | Structured guidance with actionable steps |
| **Tool Result** | String-based responses | Typed `MCPResult` with success/error states |

---

## Architecture Comparison

### Legacy Architecture (Pre-v0.4.0)

```rust
#[async_trait]
pub trait BuiltinMCPServer {
    fn tools(&self) -> Vec<MCPTool>;
    
    // ❌ Returns MCPResponse directly
    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse;
}
```

**Problems:**
- Mixed success/error handling in response construction
- Inconsistent error message formatting
- No guidance for AI agents on recovery steps
- Each server implemented error handling differently

### New Architecture (v0.4.0+)

```rust
#[async_trait]
pub trait BuiltinMCPServer {
    fn tools(&self) -> Vec<MCPTool>;
    
    // ✅ Returns Result<MCPResult, String>
    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String>;
    
    // ✅ Metadata and context support
    fn metadata(&self) -> BuiltinServerMetadata;
    async fn get_service_context(&self, options: Option<&Value>) -> ServiceContext;
}
```

**Benefits:**
- Separation of concerns: Result wrapping happens at registry level
- Standardized error guidance system (`error_guidance.rs`)
- Typed success/error states with `MCPResult`
- UI metadata support for better frontend integration

---

## Tool Description

### Legacy Pattern: Minimal Schema

```rust
// ❌ Legacy: Minimal description, no examples, unclear constraints
fn create_navigate_tool() -> MCPTool {
    MCPTool {
        name: "navigateToUrl".to_string(),
        description: "Navigate to a URL".to_string(), // Too brief
        input_schema: JSONSchema {
            schema_type: JSONSchemaType::Object {
                properties: Some({
                    let mut props = HashMap::new();
                    props.insert(
                        "url".to_string(),
                        JSONSchema {
                            schema_type: JSONSchemaType::String {
                                min_length: None, // No validation hints
                                max_length: None,
                                pattern: None,
                                format: None,
                            },
                            description: Some("URL".to_string()), // Too brief
                            examples: None, // ❌ No examples
                            // ...
                        },
                    );
                    props
                }),
                required: Some(vec!["url".to_string()]),
            },
            // ...
        },
    }
}
```

**Issues:**
- Unclear to AI agents what constitutes a valid URL
- No examples to guide usage
- No format constraints
- Brief descriptions don't explain edge cases

### New Pattern: Rich Schema with Examples

```rust
// ✅ New: Detailed description, examples, format constraints
fn create_navigate_tool() -> MCPTool {
    use crate::mcp::schema_builder::*;
    
    MCPTool {
        name: "navigateToUrl".to_string(),
        title: Some("Navigate to URL".to_string()), // Human-friendly title
        description: concat!(
            "Navigate the browser to a specific URL. ",
            "The URL must include the protocol (http:// or https://). ",
            "This will load the page and wait for basic navigation to complete. ",
            "Use extractWebContent after navigation to see the page structure."
        ).to_string(), // Detailed with usage hints
        input_schema: object_schema()
            .required_property(
                "session_id",
                string_schema()
                    .description("Unique identifier for the browser session")
                    .example("browser-session-abc123")
            )
            .required_property(
                "url",
                string_schema()
                    .description("Full URL including protocol (http:// or https://)")
                    .format("uri") // ✅ Format constraint
                    .min_length(1)
                    .max_length(2048)
                    .example("https://example.com")
                    .example("http://localhost:3000/dashboard")
            )
            .build(),
    }
}
```

**Benefits:**
- AI agents understand valid input format
- Examples guide correct usage
- Format constraints enable client-side validation
- Detailed descriptions reduce trial-and-error

---

## Input Validation

### Legacy Pattern: Manual String Checks

```rust
// ❌ Legacy: Ad-hoc validation with inconsistent error messages
async fn navigate_to_url(&self, args: Value) -> MCPResponse {
    // Manual extraction and validation
    let url = match args.get("url") {
        Some(Value::String(s)) if !s.is_empty() => s,
        _ => return MCPResponse::error("URL is required"), // ❌ No guidance
    };
    
    // No format validation
    let service = match self.get_browser_service() {
        Ok(s) => s,
        Err(e) => return MCPResponse::error(&e), // ❌ No recovery steps
    };
    
    match service.navigate(url).await {
        Ok(_) => MCPResponse::success("Navigated successfully"),
        Err(e) => MCPResponse::error(&format!("Navigation failed: {}", e)), // ❌ No actionable guidance
    }
}
```

**Problems:**
- Repetitive validation code across all tools
- Inconsistent error message format
- No actionable guidance for AI agents
- Type coercion errors not handled uniformly

### New Pattern: Schema + Structured Errors

```rust
// ✅ New: Schema-driven validation with error guidance
async fn navigate_to_url(&self, args: Value) -> Result<MCPResult, String> {
    use crate::mcp::builtin::error_guidance::*;
    
    // Extract with clear error handling
    let session_id = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "session_id is required".to_string())?;
    
    let url = args
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "url is required".to_string())?;
    
    // Validate URL format (schema already documents this)
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Ok(invalid_input_error(
            "url must start with http:// or https://",
            vec![
                "Include the protocol: https://example.com",
                "Avoid relative URLs like /path or example.com",
            ],
            ToolGroup::Browser,
        ));
    }
    
    // Get service with structured error
    let service = self.get_browser_service().map_err(|e| 
        format!("Failed to get browser service: {}", e)
    )?;
    
    // Execute operation
    match service.navigate(session_id, url).await {
        Ok(_) => Ok(success_with_hint(
            &format!("✓ Navigated to {}", url),
            vec![
                "Use extractWebContent to view page structure",
                "Use listInteractable to see clickable elements",
            ],
            ToolGroup::Browser,
        )),
        Err(e) => Ok(Self::handle_browser_op_error(
            "navigateToUrl",
            e.to_string(),
            vec![
                "Check if the URL is accessible from your network",
                "Try createSession to start a fresh browser session",
                "Use extractWebContent on the current page to see errors",
            ],
        )),
    }
}
```

**Benefits:**
- Schema serves as validation contract
- Consistent error message structure
- Actionable recovery steps for AI agents
- Success hints guide next steps

---

## Error Handling

### Legacy Pattern: Raw Error Strings

```rust
// ❌ Legacy: No structure, no guidance, inconsistent format
async fn create_session(&self, args: Value) -> MCPResponse {
    let service = match self.get_browser_service() {
        Ok(s) => s,
        Err(e) => return MCPResponse::error(&e), // Just the error string
    };
    
    match service.create_session().await {
        Ok(session_id) => MCPResponse::success(&format!("Session created: {}", session_id)),
        Err(e) => MCPResponse::error(&format!("Failed to create session: {}", e)),
        // ❌ No recovery guidance
        // ❌ No visual markers
        // ❌ No next steps
    }
}
```

**Problems:**
- AI agents don't know what to do when errors occur
- Inconsistent error format across tools
- No visual markers for quick scanning
- Internal errors leak implementation details

### New Pattern: Structured Error Guidance

```rust
// ✅ New: Structured errors with actionable guidance
async fn create_session(&self, args: Value) -> Result<MCPResult, String> {
    use crate::mcp::builtin::error_guidance::*;
    
    let service = self.get_browser_service()
        .map_err(|e| format!("Browser service unavailable: {}", e))?;
    
    match service.create_session().await {
        Ok(session_id) => {
            // ✅ Success with next steps
            Ok(success_with_hint(
                &format!("✓ Browser session created: {}", session_id),
                vec![
                    "Use navigateToUrl to load a page",
                    "Use extractWebContent to see page structure",
                    "Remember the session_id for subsequent commands",
                ],
                ToolGroup::Browser,
            ))
        }
        Err(e) => {
            // ✅ Structured error with recovery steps
            Ok(operation_failed_error(
                "createSession",
                &e.to_string(),
                vec![
                    "Check if Chrome/Chromium is installed on the system",
                    "Verify no other browser automation is running",
                    "Try restarting the application if the issue persists",
                ],
                ToolGroup::Browser,
            ))
        }
    }
}
```

**Error Guidance Module (`error_guidance.rs`):**

```rust
/// Standardized error constructors with guidance
pub fn invalid_input_error(
    message: &str,
    guidance: Vec<&str>,
    tool_group: ToolGroup,
) -> MCPResult {
    let guidance_strings: Vec<String> = guidance.iter().map(|s| s.to_string()).collect();
    ErrorGuidance::with_guidance(
        ErrorCategory::InvalidInput,
        message,
        guidance_strings,
        tool_group,
    )
    .to_mcp_result()
}

pub fn missing_param_error(param_name: &str, tool_group: ToolGroup) -> MCPResult {
    let message = format!("Missing required parameter: {}", param_name);
    ErrorGuidance::new(ErrorCategory::MissingRequiredParam, message, tool_group)
        .to_mcp_result()
}

pub fn operation_failed_error(
    operation: &str,
    error_message: &str,
    guidance: Vec<&str>,
    tool_group: ToolGroup,
) -> MCPResult {
    let message = format!("Operation '{}' failed: {}", operation, error_message);
    let guidance_strings: Vec<String> = guidance.iter().map(|s| s.to_string()).collect();
    ErrorGuidance::with_guidance(
        ErrorCategory::OperationFailed,
        message,
        guidance_strings,
        tool_group,
    )
    .to_mcp_result()
}
```

**Error Message Format:**

```
✗ Operation 'navigateToUrl' failed: Connection timeout

💡 Next Steps:
1. Check if the URL is accessible from your network
2. Try createSession to start a fresh browser session  
3. Use extractWebContent on the current page to see errors
```

**Benefits:**
- Visual markers (✓, ✗, 💡) for quick scanning
- Numbered recovery steps
- Tool group isolation (Browser tools suggest browser tools)
- Consistent formatting across all tool groups
- AI agents can automatically retry with guidance

---

## Tool Result Format

### Legacy Pattern: Untyped MCPResponse

```rust
// ❌ Legacy: Direct response construction, mixed concerns
pub struct MCPResponse {
    pub content: Vec<MCPContent>,
    pub is_error: Option<bool>,
}

impl MCPResponse {
    pub fn success(message: &str) -> Self {
        Self {
            content: vec![MCPContent::text(message)],
            is_error: Some(false),
        }
    }
    
    pub fn error(message: &str) -> Self {
        Self {
            content: vec![MCPContent::text(message)],
            is_error: Some(true),
        }
    }
}

// Tool implementation
async fn some_tool(&self, args: Value) -> MCPResponse {
    // Direct response construction
    MCPResponse::success("Done")
}
```

**Problems:**
- No type safety for success/error distinction
- No structured data support
- Mixed concerns: tool logic + response formatting
- Hard to add metadata or hints

### New Pattern: Typed MCPResult

```rust
// ✅ New: Typed result with clear success/error states
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MCPResult {
    Success {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        hints: Option<Vec<String>>,
    },
    Error {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        category: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        guidance: Option<Vec<String>>,
    },
}

impl MCPResult {
    pub fn success(message: &str) -> Self {
        Self::Success {
            message: message.to_string(),
            data: None,
            hints: None,
        }
    }
    
    pub fn success_with_data(message: &str, data: Value) -> Self {
        Self::Success {
            message: message.to_string(),
            data: Some(data),
            hints: None,
        }
    }
    
    pub fn error(message: &str) -> Self {
        Self::Error {
            message: message.to_string(),
            category: None,
            guidance: None,
        }
    }
}

// Tool implementation
async fn some_tool(&self, args: Value) -> Result<MCPResult, String> {
    // Type-safe result construction
    Ok(MCPResult::success_with_data(
        "✓ Operation completed",
        json!({ "count": 5 })
    ))
}
```

**Registry-Level Wrapping:**

```rust
// BuiltinServerRegistry wraps Result into MCPResponse
pub async fn call_tool(&self, full_tool_name: &str, args: Value) -> MCPResponse {
    // ... find server and tool ...
    
    match server.call_tool(tool_name, args).await {
        Ok(result) => {
            // ✅ Convert MCPResult to MCPResponse
            match result {
                MCPResult::Success { message, data, hints } => {
                    let mut content = vec![MCPContent::text(&message)];
                    if let Some(data) = data {
                        content.push(MCPContent::data(data));
                    }
                    if let Some(hints) = hints {
                        content.push(MCPContent::hints(hints));
                    }
                    MCPResponse {
                        content,
                        is_error: Some(false),
                    }
                }
                MCPResult::Error { message, .. } => {
                    MCPResponse {
                        content: vec![MCPContent::text(&message)],
                        is_error: Some(true),
                    }
                }
            }
        }
        Err(e) => {
            // Internal error (should be rare)
            MCPResponse::error(&format!("Internal error: {}", e))
        }
    }
}
```

**Benefits:**
- Type safety: Success and Error are distinct types
- Structured data in success responses
- Hints field for AI guidance
- Separation of concerns: tools return Result, registry handles wrapping
- Easier to add new metadata fields

---

## Migration Checklist

### For Server Implementers

#### Phase 1: Update Trait Implementation

- [ ] Change `call_tool` return type from `MCPResponse` to `Result<MCPResult, String>`
- [ ] Update all tool methods to return `Result<MCPResult, String>`
- [ ] Remove direct `MCPResponse` construction

**Before:**
```rust
async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
    match tool_name {
        "my_tool" => self.my_tool(args).await,
        _ => MCPResponse::error("Unknown tool"),
    }
}
```

**After:**
```rust
async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
    match tool_name {
        "my_tool" => self.my_tool(args).await,
        _ => Err(format!("Unknown tool: {}", tool_name)),
    }
}
```

#### Phase 2: Enrich Tool Descriptions

- [ ] Add `title` field to all tools
- [ ] Expand descriptions with usage hints and edge cases
- [ ] Add examples for all parameters (at least 2 per param)
- [ ] Add format constraints (`format`, `min_length`, `max_length`, `pattern`)
- [ ] Document parameter relationships in descriptions

**Example Checklist for One Tool:**
```rust
✅ Title: Human-friendly name
✅ Description: 2-3 sentences explaining purpose, constraints, and common use cases
✅ All parameters have:
    ✅ Clear description
    ✅ At least 2 examples
    ✅ Format/pattern constraints
    ✅ Min/max length where applicable
✅ Required parameters clearly marked
✅ Related tools mentioned in description
```

#### Phase 3: Implement Error Guidance

- [ ] Import `error_guidance` module
- [ ] Replace raw error strings with structured errors
- [ ] Add 2-3 actionable recovery steps per error
- [ ] Use appropriate `ToolGroup` for your server
- [ ] Test error messages with AI agents

**Migration Pattern:**
```rust
// ❌ Before
Err(e) => MCPResponse::error(&format!("Failed: {}", e))

// ✅ After
Err(e) => Ok(operation_failed_error(
    "operation_name",
    &e.to_string(),
    vec![
        "Try alternative_tool to check state",
        "Verify parameter_name is correct",
        "Use list_tool to see available resources",
    ],
    ToolGroup::YourGroup,
))
```

#### Phase 4: Add Success Hints

- [ ] Convert plain success messages to structured results
- [ ] Add next-step hints for workflow guidance
- [ ] Include data in success responses where applicable

**Before:**
```rust
Ok(MCPResponse::success("Done"))
```

**After:**
```rust
Ok(success_with_hint(
    "✓ Operation completed successfully",
    vec![
        "Use next_tool to continue the workflow",
        "Check list_tool to verify the changes",
    ],
    ToolGroup::YourGroup,
))
```

#### Phase 5: Testing

- [ ] Unit tests for all tool methods
- [ ] Integration tests with actual tool calls
- [ ] Error scenario coverage
- [ ] AI agent testing with real workflows
- [ ] Documentation review

---

## Code Examples

### Complete Legacy Tool

```rust
// ❌ Legacy: planning/mod.rs (old pattern)
async fn add_todo(&self, args: Value) -> MCPResponse {
    let title = match args.get("title").and_then(|v| v.as_str()) {
        Some(t) if !t.is_empty() => t,
        _ => return MCPResponse::error("Title is required"),
    };
    
    let priority = args
        .get("priority")
        .and_then(|v| v.as_str())
        .unwrap_or("medium");
    
    let parent_id = args.get("parent_id").and_then(|v| v.as_str());
    
    match self.state.add_todo(title, priority, parent_id) {
        Ok(todo) => MCPResponse::success(&format!(
            "Todo added: {} (ID: {})",
            todo.title, todo.id
        )),
        Err(e) => MCPResponse::error(&format!("Failed to add todo: {}", e)),
    }
}
```

### Complete New Tool

```rust
// ✅ New: planning/mod.rs (new pattern)
async fn add_todo(&self, args: Value) -> Result<MCPResult, String> {
    use crate::mcp::builtin::error_guidance::*;
    
    // Extract and validate parameters
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "title is required".to_string())?;
    
    if title.is_empty() {
        return Ok(invalid_input_error(
            "title cannot be empty",
            vec![
                "Provide a descriptive title for the todo",
                "Use list_todos to see existing todos for reference",
            ],
            ToolGroup::Planning,
        ));
    }
    
    let priority = args
        .get("priority")
        .and_then(|v| v.as_str())
        .unwrap_or("medium");
    
    // Validate priority
    if !["low", "medium", "high"].contains(&priority) {
        return Ok(invalid_input_error(
            &format!("Invalid priority: {}", priority),
            vec![
                "Priority must be one of: low, medium, high",
                "Use medium as default if unsure",
            ],
            ToolGroup::Planning,
        ));
    }
    
    let parent_id = args.get("parent_id").and_then(|v| v.as_str());
    
    // Check nesting depth if parent_id is provided
    if let Some(pid) = parent_id {
        if let Ok(state) = self.state.read() {
            if state.check_nesting_depth(pid) >= 2 {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::NestingTooDeep,
                    "Cannot nest todos more than 2 levels deep",
                    vec![
                        "Create as top-level todo instead".to_string(),
                        "Attach to a different parent that has no parent".to_string(),
                        "Use list_todos to see the current hierarchy".to_string(),
                    ],
                    ToolGroup::Planning,
                ).to_mcp_result());
            }
        }
    }
    
    // Execute operation
    match self.state.add_todo(title, priority, parent_id) {
        Ok(todo) => {
            Ok(success_with_data(
                &format!("✓ Todo added: {}", todo.title),
                json!({
                    "id": todo.id,
                    "title": todo.title,
                    "priority": todo.priority,
                    "status": "pending",
                    "parent_id": todo.parent_id,
                }),
            ))
        }
        Err(e) => {
            // Check for specific error types
            if e.contains("duplicate") {
                Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::DuplicateResource,
                    &format!("A todo with title '{}' already exists", title),
                    vec![
                        "Use a different title for the new todo".to_string(),
                        "Use update_todo to modify the existing todo".to_string(),
                        "Use list_todos to see all existing todos".to_string(),
                    ],
                    ToolGroup::Planning,
                ).to_mcp_result())
            } else {
                Ok(operation_failed_error(
                    "add_todo",
                    &e,
                    vec![
                        "Verify the parent_id exists if provided",
                        "Use getCurrentState to check planning state",
                        "Try without parent_id to create top-level todo",
                    ],
                    ToolGroup::Planning,
                ))
            }
        }
    }
}
```

---

## Testing Comparison

### Legacy Testing: Response String Matching

```rust
// ❌ Legacy: Brittle string matching
#[tokio::test]
async fn test_add_todo_legacy() {
    let server = PlanningServer::new("test-session".to_string());
    
    // Success case
    let result = server.call_tool("add_todo", json!({
        "title": "Test task"
    })).await;
    
    assert!(result.content[0].text.contains("Todo added"));
    assert_eq!(result.is_error, Some(false));
    
    // Error case
    let result = server.call_tool("add_todo", json!({})).await;
    assert!(result.content[0].text.contains("required")); // Brittle!
    assert_eq!(result.is_error, Some(true));
}
```

### New Testing: Typed Result Matching

```rust
// ✅ New: Type-safe matching
#[tokio::test]
async fn test_add_todo_new() {
    let server = PlanningServer::new("test-session".to_string());
    
    // Success case
    let result = server.call_tool("add_todo", json!({
        "title": "Test task",
        "priority": "high"
    })).await;
    
    match result {
        Ok(MCPResult::Success { message, data, .. }) => {
            assert!(message.contains("✓"));
            assert!(message.contains("Test task"));
            
            // Verify structured data
            let data = data.expect("Should have data");
            assert_eq!(data["title"], "Test task");
            assert_eq!(data["priority"], "high");
            assert_eq!(data["status"], "pending");
        }
        _ => panic!("Expected success result"),
    }
    
    // Error case: Missing title
    let result = server.call_tool("add_todo", json!({})).await;
    match result {
        Ok(MCPResult::Error { message, category, guidance }) => {
            assert!(message.contains("✗"));
            assert!(message.contains("required"));
            assert_eq!(category, Some("MissingRequiredParam".to_string()));
            assert!(guidance.is_some());
            
            let guidance_steps = guidance.unwrap();
            assert!(!guidance_steps.is_empty());
        }
        _ => panic!("Expected error result"),
    }
    
    // Error case: Invalid priority
    let result = server.call_tool("add_todo", json!({
        "title": "Test",
        "priority": "critical" // Invalid
    })).await;
    
    match result {
        Ok(MCPResult::Error { message, guidance, .. }) => {
            assert!(message.contains("Invalid priority"));
            
            let steps = guidance.expect("Should have guidance");
            assert!(steps.iter().any(|s| s.contains("low, medium, high")));
        }
        _ => panic!("Expected error result"),
    }
}
```

---

## Key Differences Summary

| Aspect | Legacy | New | Impact |
|--------|--------|-----|--------|
| **Return Type** | `MCPResponse` | `Result<MCPResult, String>` | Type safety, cleaner separation |
| **Error Format** | Plain strings | Structured with guidance | AI agents can auto-recover |
| **Success Messages** | Plain text | Text + data + hints | Richer workflow guidance |
| **Tool Descriptions** | Brief | Detailed with examples | Better AI understanding |
| **Input Validation** | Ad-hoc | Schema-driven | Consistent validation |
| **Error Guidance** | None | 2-3 actionable steps | Reduced trial-and-error |
| **Visual Markers** | None | ✓, ✗, 💡 | Quick scanning |
| **Testing** | String matching | Type matching | More maintainable |

---

## Migration Priority

**High Priority Servers** (User-facing, frequently used):
1. ✅ `browser` - Already migrated
2. ✅ `planning` - Already migrated
3. ⏳ `workspace` - In progress
4. ⏳ `assistant` - In progress

**Medium Priority Servers** (Backend, less frequent):
5. `content_store`
6. `knowledge`
7. `playbook`

**Low Priority Servers** (Utility, internal):
8. `ui`
9. `mcp_manager`
10. `bootstrap`

---

## Resources

- [error_guidance.rs](/home/fritzprix/my_works/libr-agent/src-tauri/src/mcp/builtin/error_guidance.rs) - Error guidance system
- [browser.rs](/home/fritzprix/my_works/libr-agent/src-tauri/src/mcp/builtin/browser.rs) - Complete new pattern example
- [planning/mod.rs](/home/fritzprix/my_works/libr-agent/src-tauri/src/mcp/builtin/planning/mod.rs) - Planning server migration
- [mod.rs](/home/fritzprix/my_works/libr-agent/src-tauri/src/mcp/builtin/mod.rs) - BuiltinMCPServer trait definition
- [Built-in Tool Development Guide](/home/fritzprix/my_works/libr-agent/src-tauri/src/mcp/builtin/README.md) - Original development guide

---

## FAQ

### Q: Can I mix legacy and new patterns?

**A:** Yes, during migration. The registry handles both patterns. However, aim to fully migrate each server for consistency.

### Q: How do I decide what guidance to provide?

**A:** Think like the AI agent:
1. What would I try next if this fails?
2. What tool helps me verify state?
3. What common mistakes lead to this error?

### Q: Should every error have custom guidance?

**A:** No. Use default guidance from `ErrorCategory` for common cases. Add custom guidance for domain-specific errors.

### Q: How many examples should each parameter have?

**A:** Minimum 2, ideally 3-4 covering:
- Typical use case
- Edge case
- Different formats (if applicable)
- Related use case

### Q: Can success results have multiple hints?

**A:** Yes, but keep it to 2-3 most relevant next steps. Too many hints overwhelm the AI agent.

---

**Last Updated**: December 31, 2025  
**Maintainer**: LibrAgent Core Team  
**Feedback**: Open an issue in the repository
