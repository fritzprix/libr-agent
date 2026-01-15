# Refactoring Plan: Browser Tool Compliance & Best Practices

## 1. Needs

The current browser tool implementation (`src-tauri/src/mcp/builtin/browser/`) functions correctly but violates several best practices regarding AI-compatible language and proactive validation.

**Key Drivers:**

- **AI Comprehension**: Tool descriptions use human-centric or ambiguous language ("from memory", "other tools") which confuses AI agents.
- **Robustness**: Missing proactive validation for URLs can lead to avoidable errors.
- **Consistency**: Success/Error guidance varies in specificity and completeness.

## 2. Current State / Problems

### 2.1 AI-Incompatible Tool Descriptions

- **Problem**: `createSession` and other tool descriptions use negative constraints like "DO NOT use session IDs from previous attempts" which implies a distinction between "memory" and "context" that doesn't exist for AI.
- **Problem**: Descriptions are too brief (e.g., `navigateBack`) and lack workflow context.
- **Problem**: Conditional behavior (pagination, auto-merge) is not clearly explained in `extractWebContent`.

### 2.2 Missing Proactive Validation

- **Problem**: `navigate_to_url` only checks for http/https prefix but lacks:
  - URL length limits (DoS prevention).
  - Explicit blocking of `file://` or other protocols.
  - Basic encoding validation.

### 2.3 Guidance Quality

- **Problem**: Success messages for navigation often omit the Session ID, forcing the agent to remember it from context blindly.
- **Problem**: Error guidance is sometimes generic ("Verify session is active") instead of actionable ("Use getCurrentUrl to check status").

## 3. Related Code Structure (Bird's Eye View)

The browser module is organized by feature:

- `mod.rs`:
  - Defines `BrowserServer` struct and trait implementation.
  - Contains all `MCPTool` definitions (descriptions, schemas).
  - Handles routing in `call_tool`.
- `navigation.rs`:
  - `navigate_to_url`, `navigate_back`, `navigate_forward`.
  - Usage of `handle_browser_op_error`.
- `session.rs`:
  - `create_session`, `close_session`.
  - Manages session lifecycle and cache clearing.
- `interaction.rs`:
  - `click_element`, `input_text`, `scroll_page`.
  - Logic for executing JS and interpreting results.
- `content.rs`:
  - `extract_web_content`, `read_web_content`.
  - Handles pagination and markdown conversion.

## 4. Target State / Resolution Criteria

### 4.1 Success Criteria

- **Language**: All tool descriptions use "Extract", "Use exact value", "Reference" instead of "Copy", "Remember", "From memory".
- **Validation**: invalid inputs (long URLs, `file://`) are caught before service calls with clear error messages.
- **Clarity**: Success messages for navigation explicitly state the current Session ID.
- **Workflows**: All tools include a "WORKFLOW" or "MANDATORY WORKFLOW" section in their description.

### 4.2 Expected Behavior

- AI agents correctly chain `createSession` → `navigateToUrl` without hallucinating IDs.
- Agents correctly handle pagination conditions in `extractWebContent`.
- Users see specific, actionable error messages when validation fails.

## 5. Code to be Modified

### 5.1 `mod.rs`: Tool Descriptions

Refactor `MCPTool` definitions to use AI-compatible patterns.

**Before (Example):**

```rust
description: "⚠️ CRITICAL: Browser session ID returned by createSession.\n\nWORKFLOW:\n1. Call createSession FIRST to get a session ID\n2. Use the exact session ID from createSession response\n3. DO NOT use session IDs from previous attempts or other tools"
```

**After (Example):**

```rust
description: "⚠️ CRITICAL: Browser session ID returned by createSession.

MANDATORY WORKFLOW:
1. Call createSession FIRST to get a session ID
2. Extract the exact session ID from createSession response
3. Use the extracted session ID as this parameter

❌ NEVER reconstruct session IDs or assume previous IDs still work
✅ ALWAYS use the session ID exactly as returned by createSession"
```

### 5.2 `navigation.rs`: Proactive Validation

Add validation logic to `navigate_to_url`.

**Snippet:**

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
        "Local file URLs are not supported for security. Use http:// or https:// URLs only",
        ToolGroup::Browser,
    ));
}

if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("about:") {
    // ... existing check ...
}
```

### 5.3 `navigation.rs`: Enhanced Success Tips

Include Session ID in success messages.

**Snippet:**

```rust
let hint = SuccessHint::new(
    format!("Navigated to {} (Session: {})", url, session_id), // Added Session ID
    suggestions
);
```

## 6. Reusable Related Code

- **Error Guidance Module**: `src-tauri/src/mcp/builtin/error_guidance.rs`
  - Use `invalid_input_error`, `SuccessHint`, `ErrorCategory`.
- **Browser Service**: `src-tauri/src/services/interactive_browser.rs` (Backend implementation).

## 7. Test Code Guide

Since this is largely a logic/validation refactoring, unit tests should be added/updated in a new `tests` module within `src-tauri/src/mcp/builtin/browser/mod.rs` (or separate file if preferred):

1.  **Test Tool Description Language**:
    - Verify no prohibited words ("copy", "memory") exist in `tools()` output.
2.  **Test Validation Logic**:
    - Call `navigate_to_url` with >2048 char URL -> Expect `InvalidInput`.
    - Call `navigate_to_url` with `file://...` -> Expect `InvalidInput`.
3.  **Test Error Formatting**:
    - Verify `handle_browser_op_error` returns correct guidance for timeout vs 403.

## 8. Clarification Q-list

- **Q1**: Are there any strict constraints on URL length other than 2048? (Defaulting to 2048 as safe standard).
  - A1: Nope! It's okay with 2048
- **Q2**: Should `about:blank` be strictly allowed or discouraged? (Currently allowed for initialization).
  - about:blank is causing issue,
    - if input url is about:blank, the browser will be directed to google.com
    - if input url is not given, the browser will be directed to google.com
