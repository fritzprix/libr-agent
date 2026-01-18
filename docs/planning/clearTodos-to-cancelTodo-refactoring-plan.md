# Refactoring Plan: Planning Module - `clearTodos` → `cancelTodo`

**Date:** January 16, 2026
**Branch:** dev/0.4.0
**Estimated Time:** 95 minutes (1h 35min)
**Status:** ✅ Approved - Ready for Implementation

---

## Executive Summary

**Goal:** Rename `clearTodos` to `cancelTodo` for better semantic clarity, fix missing completion summary display, and align with AI-compatible best practices.

**Approach:** Follow **Approach 1 (Simple)** - minimal changes, no schema migration, maximum clarity.

**Key Changes:**

1. Rename tool: `clearTodos` → `cancelTodo`
2. Fix context display to show completion summaries
3. Update tool descriptions with AI-compatible language
4. Implement unified error handling with visual markers
5. Add comprehensive unit tests

---

## Phase 1: Rename Tool (10 minutes)

### 1.1 Update Tool Registration

**File:** `src-tauri/src/mcp/builtin/planning/mod.rs`

**Changes:**

- Rename tool from `clearTodos` to `cancelTodo`
- Update title from "Clear Todos" to "Cancel Todo"
- Update match arm in `call_tool`

### 1.2 Rename Function

**File:** `src-tauri/src/mcp/builtin/planning/todos.rs`

**Changes:**

- Rename function: `clear_todos` → `cancel_todo`
- Update all log messages to use "Cancelled" instead of "Cleared"
- Update success messages with new terminology

---

## Phase 2: Fix Context Display Bug (15 minutes)

### 2.1 Show Completion Summary in Context

**File:** `src-tauri/src/mcp/builtin/planning/context.rs`

**Changes:**

- Add "Summary" column to checked todos table
- Extract summary from description field
- Truncate summary to 50 chars max to prevent table overflow
- Escape pipe characters properly

---

## Phase 3: Update Tool Descriptions (15 minutes) ⚠️ CRITICAL

### 3.1 AI-Compatible Tool Descriptions

**File:** `src-tauri/src/mcp/builtin/planning/mod.rs`

**Changes:**

#### `cancelTodo` Tool Description:

```
Permanently remove specific todos by ID or index. Use this tool when:
• Task was created incorrectly
• Requirements changed and task is no longer needed
• Task duplicates another todo

⚠️ IMPORTANT: This operation is irreversible
❌ DO NOT use for completed tasks - use checkTodo instead to preserve completion history
✓ Use cancelTodo only for tasks that should not exist

If no parameters provided, cancels ALL todos (complete reset).
```

#### `cancelTodo` Parameter Descriptions:

- `id`: "Todo ID to cancel. Extract from getCurrentState response."
- `index`: "0-based index of todo to cancel. Extract from getCurrentState response."

#### `checkTodo` Tool Description:

```
Mark a todo item as completed or unchecked. Checked todos remain in the list for progress tracking and can be unchecked later. Optionally provide a summary of how the task was completed (e.g., 'Fixed with PR #42', 'Resolved in commit abc123').
```

#### `checkTodo` Summary Parameter:

```
Completion summary documenting how the task was resolved (e.g., 'Fixed with PR #42', 'Resolved in commit abc123', 'Documentation updated'). This summary is displayed in service context and helps track progress across sessions.
```

---

## Phase 4: Update Error Handling (15 minutes) ⚠️ CRITICAL

### 4.1 Implement Unified Error System

**File:** `src-tauri/src/mcp/builtin/planning/todos.rs`

**Changes:**

#### Import Error Guidance:

```rust
use crate::mcp::builtin::error_guidance::{
    not_found_error,
    invalid_input_error,
    ErrorGuidance,
    ErrorCategory,
    SuccessHint,
    ToolGroup,
};
```

#### Update `cancel_todo` Error Responses:

1. **Todo Not Found Error:**

```rust
return Ok(not_found_error(
    "Todo",
    &format!("ID {} or index {}", id_str, index_str),
    ToolGroup::Planning,
));
```

2. **Invalid Parameter Error:**

```rust
return Ok(invalid_input_error(
    "Invalid 'id'. Must be >= 1",
    ToolGroup::Planning,
));
```

3. **Success Response with Visual Markers:**

```rust
let hint = SuccessHint::new(
    format!("✓ Cancelled {} todo(s)", deleted_count),
    vec![
        "All todos cancelled. Use 'getCurrentState' to verify empty state.".to_string(),
        "Use 'createGoal' to start a new objective.".to_string(),
    ],
);
```

#### Update `check_todo` Messages:

```rust
let action_desc = if checked {
    "checked (completed)"
} else {
    "unchecked (reopened)"
};
```

---

## Phase 5: Validation & Testing (30 minutes)

### 5.1 Manual Testing Checklist

- [ ] **Rename Verification**
  - Tool appears as `cancelTodo` in MCP tool list
  - Frontend calls `cancelTodo` successfully
  - Old `clearTodos` name removed from codebase

- [ ] **Functionality Tests**
  - Cancel single todo by ID: `cancelTodo(id: 42)`
  - Cancel single todo by index: `cancelTodo(index: 0)`
  - Cancel all todos: `cancelTodo()` (no params)
  - Check todo with summary: `checkTodo(id: 42, summary: "Fixed with PR #123")`

- [ ] **Context Display Tests**
  - Checked todos show summary in context table
  - Summary displays correctly with special characters
  - Summary truncation works (50 char limit)
  - Empty summary shows "-" instead of blank

- [ ] **Error Response Tests**
  - Errors contain visual markers (✗, 💡)
  - Errors provide 2-3 actionable recovery steps
  - Errors only suggest Planning tool group tools
  - Success responses contain visual markers (✓, 💡)

### 5.2 Unit Tests

**File:** `src-tauri/src/mcp/builtin/planning/todos.rs`

Add tests for:

- Error formatting with visual markers
- Tool group isolation (Planning tools only)
- Success response format
- Function signature compliance

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::builtin::error_guidance::ToolGroup;

    #[test]
    fn test_cancel_todo_error_has_visual_markers() {
        let error = not_found_error("Todo", "999", ToolGroup::Planning);
        assert_eq!(error.is_error, Some(true));

        if let Some(content) = &error.content {
            if let Some(MCPContent::Text { text }) = content.first() {
                assert!(text.contains("✗"), "Error should have ✗ marker");
                assert!(text.contains("💡 Next Steps:"), "Error should have guidance marker");
            }
        }
    }

    #[test]
    fn test_cancel_todo_error_suggests_planning_tools_only() {
        let error = not_found_error("Todo", "999", ToolGroup::Planning);

        if let Some(content) = &error.content {
            if let Some(MCPContent::Text { text }) = content.first() {
                assert!(text.contains("getCurrentState") || text.contains("createGoal"),
                    "Should suggest Planning tools");
                assert!(!text.contains("navigateToUrl"), "Should not suggest Browser tools");
            }
        }
    }

    #[test]
    fn test_cancel_todo_success_response_format() {
        let hint = SuccessHint::new(
            "✓ Cancelled 2 todos".to_string(),
            vec!["Next step here".to_string()],
        );

        let result = hint.to_mcp_result();
        assert_eq!(result.is_error, Some(false));

        if let Some(content) = &result.content {
            if let Some(MCPContent::Text { text }) = content.first() {
                assert!(text.contains("✓"), "Success should have ✓ marker");
                assert!(text.contains("💡"), "Success should have guidance marker");
            }
        }
    }
}
```

### 5.3 Build Validation

```bash
# Run full validation pipeline
pnpm refactor:validate

# Expected output:
# ✓ ESLint checks pass
# ✓ Prettier formatting pass
# ✓ Rust formatting pass (cargo fmt --check)
# ✓ Rust clippy pass (cargo clippy)
# ✓ Build successful (pnpm build)
# ✓ No dead code detected
```

---

## Phase 6: Documentation Updates (10 minutes)

### 6.1 Update Built-in Tools Documentation

**File:** `docs/builtin-tools.md` (if exists)

Add section documenting:

- `cancelTodo` tool purpose and usage
- `checkTodo` tool with summary field
- Semantic distinction between check vs cancel

### 6.2 Update Planning Module README

**File:** `src-tauri/src/mcp/builtin/planning/README.md` (if exists)

- Update tool name references: `clearTodos` → `cancelTodo`
- Add semantic distinction explanation
- Document summary field behavior

---

## Implementation Order

**Critical Path:**

1. **Phase 3** - AI-compatible descriptions (blocks AI agent testing)
2. **Phase 4** - Error handling with visual markers (blocks validation)
3. **Phase 1** - Rename tool (atomic change)
4. **Phase 2** - Context display with truncation (independent)
5. **Phase 5** - Comprehensive testing (depends on all above)
6. **Phase 6** - Documentation (final step)

**Parallel Work Possible:**

- Phase 1 and Phase 2 can be done simultaneously
- Phase 3 and Phase 4 can be done simultaneously

---

## Rollback Plan

If issues arise during refactoring:

1. **Git Revert:** Use `git revert` to undo specific commits
2. **Function Rename:** If `cancelTodo` causes issues, add temporary alias:
   ```rust
   // Temporary backward compatibility
   "clearTodos" => self.cancel_todo(db, session_id, params).await,
   "cancelTodo" => self.cancel_todo(db, session_id, params).await,
   ```
3. **Context Display:** If summary display breaks, revert to original format

---

## Success Criteria

- [ ] Tool renamed from `clearTodos` to `cancelTodo`
- [ ] All function calls updated consistently
- [ ] Completion summaries visible in context table with truncation
- [ ] Tool descriptions use AI-compatible language
- [ ] Error responses use unified error system with visual markers
- [ ] All unit tests pass
- [ ] Build validation passes (`pnpm refactor:validate`)
- [ ] Documentation updated
- [ ] No breaking changes to existing workflows

---

## Risk Assessment

| Risk                               | Probability | Impact | Mitigation                                                    |
| ---------------------------------- | ----------- | ------ | ------------------------------------------------------------- |
| Breaking change for existing users | Low         | Medium | Add deprecation notice, support both names temporarily        |
| Context display breaks UI          | Low         | Low    | Add fallback to "-" for empty summaries, truncate at 50 chars |
| AI agents misuse tool              | Medium      | High   | Use AI-compatible descriptions with explicit DO/DON'T         |
| Inconsistent error handling        | Low         | High   | Mandatory use of `error_guidance` functions                   |
| Missing visual markers             | Low         | Medium | Add unit tests to validate marker presence                    |

---

## Post-Implementation Tasks

1. Monitor user feedback for confusion about new name
2. Check analytics for `cancelTodo` vs `checkTodo` usage patterns
3. Validate AI agents use tool correctly in practice
4. Consider adding `restoreTodo` in future if users request undo
5. Evaluate need for soft-delete in future releases based on user requests

---

## Timeline

- **Phase 1 (Rename):** 10 minutes
- **Phase 2 (Display Fix):** 15 minutes
- **Phase 3 (AI Descriptions):** 15 minutes ⚠️ CRITICAL
- **Phase 4 (Error Handling):** 15 minutes ⚠️ CRITICAL
- **Phase 5 (Testing):** 30 minutes
- **Phase 6 (Documentation):** 10 minutes

**Total:** 95 minutes (1h 35min)

---

## Files to Modify

1. `src-tauri/src/mcp/builtin/planning/mod.rs` - Tool registration, descriptions
2. `src-tauri/src/mcp/builtin/planning/todos.rs` - Function rename, error handling
3. `src-tauri/src/mcp/builtin/planning/context.rs` - Context display fix
4. `docs/builtin-tools.md` - Documentation (if exists)
5. `src-tauri/src/mcp/builtin/planning/README.md` - Documentation (if exists)

---

## Dependencies

- ✅ `error_guidance` module exists and is working
- ✅ `SuccessHint` struct available
- ✅ `ToolGroup::Planning` defined
- ✅ Visual marker constants (✗, ✓, 💡) in use

---

## Notes

- This refactoring follows **Approach 1 (Simple)** - no database schema changes
- Planning module uses ephemeral task tracking - hard deletion is acceptable
- AI-compatible descriptions are critical for proper agent behavior
- Visual markers improve human readability and AI parsing
- Tool group isolation ensures contextually relevant error suggestions

---

**Approved By:** GitHub Copilot
**Review Date:** January 16, 2026
**Implementation Ready:** ✅ Yes
