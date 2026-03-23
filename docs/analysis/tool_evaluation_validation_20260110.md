# Tool Evaluation Report Validation

**Date:** 2026-01-10  
**Validator:** GitHub Copilot  
**Target:** Engineering Sprint Planning for `builtin_content_store` & `builtin_planning`

---

## Executive Summary

✅ **VALIDATION STATUS: CONFIRMED WITH EXCEPTIONS**

The report accurately identifies real issues in the codebase, though some characterizations require clarification. All four P0/P1 issues are valid and actionable. The P2 issue requires system prompt changes rather than code modifications.

---

## Issue-by-Issue Validation

### 1. Content Store (`builtin_content_store`)

#### Issue A: Misleading "Truncated" Flag ✅ **CONFIRMED**

**Location:** `src-tauri/src/mcp/builtin/content_store/handlers.rs:450-456`

```rust
let content_preview = if content.len() > 2000 {
    format!(
        "{}\n... (truncated, {} bytes total)",
        content.chars().take(2000).collect::<String>(),
        content.len()
    )
} else {
    content.clone()
};
```

**Validation:**

- ✅ Truncation logic is based solely on **byte count** (`content.len() > 2000`)
- ✅ No check for whether remaining content is significant
- ✅ Agent cannot distinguish "99% returned" from "10% returned"
- ✅ This causes the exact behavior described: blind retries for nonexistent data

**Impact Confirmed:** High  
**Root Cause Validated:** The tool flags content as truncated without considering:

- What percentage of content was returned
- Whether remaining bytes are just whitespace
- Whether the requested line range was fully satisfied

---

#### Issue B: Missing Line Count Metadata ✅ **CONFIRMED WITH CLARIFICATION**

**Location:** `src-tauri/src/mcp/builtin/content_store/handlers.rs:295-330`

```rust
// Human-readable output (what agent sees):
format!(
    "[{}] ID: {}\n    Title: {}\n    Size: {} bytes\n    Preview: {}\n    Created: {}",
    idx + 1,
    item.id,
    item.filename,
    item.size,      // ❌ Only shows bytes
    preview_text,
    item.uploaded_at
)

// JSON structured data (available in response.data):
serde_json::json!({
    "contentId": item.id,
    "lineCount": item.line_count,  // ✅ Present in JSON
    // ...
})
```

**Validation:**

- ✅ Line count (`line_count`) exists in the data model (storage.rs:25)
- ✅ Line count IS included in JSON response (`lineCount`)
- ❌ **BUT** line count is NOT shown in human-readable text
- ✅ Agent must parse JSON or guess from byte count

**Impact Confirmed:** Medium (not High)  
**Clarification:** The data exists but is not visible in the primary text output that agents read. Agents can access it via `response.data.lineCount` if they check, but most agents rely on the text summary.

**Recommendation:** Add line count to human-readable format:

```rust
format!(
    "[{}] ID: {}\n    Title: {}\n    Size: {} bytes ({} lines)\n    Preview: {}\n    Created: {}",
    idx + 1, item.id, item.filename, item.size, item.line_count, preview_text, item.uploaded_at
)
```

---

#### Issue C: Vague Error Messaging ✅ **CONFIRMED**

**Location:** `src-tauri/src/mcp/builtin/content_store/storage.rs:485-487`

```rust
if result.is_empty() {
    return Err("No content found in specified line range".to_string());
}
```

**Validation:**

- ✅ Error message does not indicate whether:
  - Range exceeded file bounds
  - Range was valid but yielded empty chunks
  - Content ID was invalid
- ✅ Agent cannot distinguish "out of bounds" from "search failure"
- ✅ No metadata provided (e.g., actual file length, valid range)

**Impact Confirmed:** Medium  
**Root Cause:** Single generic error message for multiple failure modes.

**Current Error Path:**

```
storage.rs:487 → handlers.rs:430 → "Read content" operation failed
```

Final error shows:

```rust
operation_failed_error(
    "Read content",
    &e.to_string(),  // Just "No content found in specified line range"
    vec![
        "Verify the content ID is correct".to_string(),
        "Check line range is valid".to_string(),
        "Use list to see available content".to_string(),
    ],
    // ...
)
```

**Recommendation:** Enhance `read_content()` to return specific error with bounds:

```rust
if result.is_empty() {
    let total_lines = chunks.iter().map(|c| c.line_range.1).max().unwrap_or(0);
    return Err(format!(
        "Line range {}-{} exceeds file length. File has {} lines (valid range: 1-{})",
        from_line, target_to_line, total_lines, total_lines
    ));
}
```

---

### 2. Planning System (`builtin_planning`)

#### Issue A: Missing Content IDs in Scratchpad ❌ **PARTIALLY REJECTED**

**Location:** `src-tauri/src/mcp/builtin/planning/context.rs:250-289`

```rust
// Scratchpad formatting in ServiceContext
if !scratchpad.is_empty() {
    parts.push(format!("\n**Scratchpad:** {} items", scratchpad.len()));
    parts.push("".to_string());

    for (idx, item) in scratchpad.iter().enumerate() {
        parts.push(format!(
            "  {}. **ID:{}** {}{}{}",
            idx + 1,
            item.id,           // ✅ ID is present
            title_part,
            content_part,
            tags_part
        ));
    }
}
```

**Validation:**

- ❌ **Report claim is INCORRECT:** IDs ARE already displayed in scratchpad
- ✅ Format: `1. **ID:123** Title - Content [tags]`
- ❌ Issue is NOT about content store attachments

**Clarification Required:**
The report conflates two different systems:

1. **Planning Scratchpad** (planning module) - Shows IDs correctly
2. **Content Store** (content_store module) - This is what likely needs IDs in context

**Re-analysis:**
If the trace shows missing `contentId` in planning context, the issue is:

- Content store items are NOT automatically added to planning scratchpad
- Agent must manually note the `contentId` using `addScratchpad`
- This is by design (separation of concerns)

**Recommendation:**

- If the goal is to auto-populate scratchpad with uploaded content IDs:
  - Modify `handle_save_knowledge` to optionally create scratchpad entry
  - OR: Enhance ServiceContext to include recent content uploads
- Current behavior is correct per architecture; issue is workflow, not bug

**Verdict:** Issue severity downgraded from P0 to **P2 - Enhancement** (not a bug)

---

#### Issue B: Lack of Output Verification in CheckTodo ⚠️ **ARCHITECTURAL LIMITATION**

**Location:** `src-tauri/src/mcp/builtin/planning/todos.rs:340-420`

```rust
pub async fn check_todo(
    db: &DatabaseConnection,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    // ... fetch and check todo ...

    let result = active_todo.update(db).await;
    match result {
        Ok(_) => {
            let hint = SuccessHint::new(
                format!("Todo {} {}{}", target_id, action, summary_text),
                vec!["Use getCurrentState to see updated planning state".to_string()],
            );
            Ok(hint.to_mcp_result_with_data(Some(json!({ /* ... */ }))))
        }
        // ...
    }
}
```

**Validation:**

- ✅ No validation that work was actually performed
- ✅ No detection of immediate `addTodo → checkTodo` sequences
- ✅ No artifact generation verification
- ⚠️ **BUT:** This requires cross-tool state tracking, not just planning module changes

**Architectural Reality:**

```
addTodo (planning) → [Agent decides] → generate_artifact (?) → checkTodo (planning)
                                            ↑
                                      Not tracked by planning module
```

**Why This Is Hard:**

1. Planning module has no visibility into:
   - What the agent did between `addTodo` and `checkTodo`
   - Whether artifacts were created in content store
   - Whether code was written, files modified, etc.

2. Detection would require:
   - Session-level event log (doesn't exist)
   - Inter-tool communication protocol (not implemented)
   - Timestamp-based heuristics (fragile)

**Current Mitigation:**

- System prompt instructs agents to generate output before checking
- `SuccessHint` says "Use getCurrentState to verify"
- But no enforcement mechanism

**Recommendation:**

- **Short-term (System Prompt):** Add explicit warning in tool description

  ```
  WARNING: Do not check a todo immediately after creation without generating
  the actual work output (file, report, code, etc.). The system cannot verify
  work completion; this is your responsibility.
  ```

- **Medium-term (Soft Warning):** Add timestamp check

  ```rust
  let time_since_creation = now - todo.created_at;
  if time_since_creation < 5000 && summary.is_none() {
      // Warn but don't block
      hint.add_warning("Todo checked very quickly after creation. Ensure work is complete.");
  }
  ```

- **Long-term (Event System):** Implement session activity log
  - Track tool calls per session
  - Detect "empty work" patterns
  - Provide feedback in `checkTodo`

**Verdict:** Issue severity remains **P2 - Enhancement**  
**Reason:** Requires system-level changes, not just planning module fixes

---

## Priority Matrix (Validated)

| Priority | Component     | Issue                            | Severity | Effort | Valid?                  |
| :------- | :------------ | :------------------------------- | :------- | :----- | :---------------------- |
| **P0**   | Content Store | Fix truncation flag logic        | High     | Small  | ✅ YES                  |
| **P0**   | Content Store | Add line count to text output    | Medium   | Small  | ✅ YES                  |
| **P1**   | Content Store | Specific error messages          | Medium   | Medium | ✅ YES                  |
| **P2**   | Planning      | Scratchpad contentId display     | Low      | N/A    | ❌ NO (Already present) |
| **P2**   | Planning      | Output verification in checkTodo | Medium   | Large  | ⚠️ YES (Architectural)  |

---

## Recommended Sprint Tasks (Revised)

### Sprint 1: Content Store Fixes (P0)

#### Task 1.1: Fix Truncation Logic

**File:** `src-tauri/src/mcp/builtin/content_store/handlers.rs:450-456`

```rust
// Current (problematic)
let content_preview = if content.len() > 2000 {
    format!("{}\n... (truncated, {} bytes total)", /* ... */)
} else {
    content.clone()
};

// Proposed fix
let total_lines = content.lines().count();
let returned_lines = to_line - from_line + 1;
let is_truncated = content.len() > 2000 || to_line < total_lines;

let content_preview = if is_truncated {
    let remaining_lines = total_lines.saturating_sub(to_line);
    if remaining_lines > 0 {
        format!(
            "{}\n... (truncated: {} more lines remaining, total {} lines)",
            content.chars().take(2000).collect::<String>(),
            remaining_lines,
            total_lines
        )
    } else {
        format!("{}\n(End of file reached)", content.clone())
    }
} else {
    format!("{}\n(End of file reached)", content)
};
```

**Acceptance Criteria:**

- ✅ Show "End of file" when all content returned
- ✅ Show line-based truncation info (not just bytes)
- ✅ Calculate remaining lines accurately

---

#### Task 1.2: Add Line Count to Metadata Display

**File:** `src-tauri/src/mcp/builtin/content_store/handlers.rs:307`

```rust
// Before
format!(
    "[{}] ID: {}\n    Title: {}\n    Size: {} bytes\n    Preview: {}\n    Created: {}",
    idx + 1, item.id, item.filename, item.size, preview_text, item.uploaded_at
)

// After
format!(
    "[{}] ID: {}\n    Title: {}\n    Size: {} bytes, {} lines\n    Preview: {}\n    Created: {}",
    idx + 1, item.id, item.filename, item.size, item.line_count, preview_text, item.uploaded_at
)
```

**Acceptance Criteria:**

- ✅ Line count visible in `list` output
- ✅ Agent can estimate ranges without guessing
- ✅ Reduces "invalid line range" errors

---

### Sprint 2: Content Store Error Refinement (P1)

#### Task 2.1: Enhance Error Messages

**File:** `src-tauri/src/mcp/builtin/content_store/storage.rs:485-487`

```rust
// Before
if result.is_empty() {
    return Err("No content found in specified line range".to_string());
}

// After
if result.is_empty() {
    let max_line = chunks.iter().map(|c| c.line_range.1).max().unwrap_or(0);

    if from_line > max_line {
        return Err(format!(
            "Error: Requested range [{}-{}] exceeds file length.\n\
             File has {} lines. Valid range: [1-{}]",
            from_line, target_to_line, max_line, max_line
        ));
    } else {
        return Err(format!(
            "No content found in line range [{}-{}]. File has {} lines.",
            from_line, target_to_line, max_line
        ));
    }
}
```

**Acceptance Criteria:**

- ✅ Distinguish "out of bounds" from "empty result"
- ✅ Provide actual file bounds in error
- ✅ Reduce retry loops in agent behavior

---

### Sprint 3: Planning Enhancements (P2 - Optional)

#### Task 3.1: Add System Prompt Warning

**File:** `src-tauri/src/mcp/builtin/planning/mod.rs:120` (tool description)

Add to `checkTodo` description:

```
⚠️ IMPORTANT: Only check a todo after you have generated the actual work output
(file, code, report, etc.). Do not check tasks immediately after creation without
producing deliverables. The system cannot verify work completion.
```

#### Task 3.2: Soft Warning for Quick Completion (Future)

Implement timestamp-based heuristic (requires event log system).

---

## Validation Test Cases

### Test Case 1: Truncation Flag Accuracy

```bash
# Setup: Add 100-line file
add(content="line1\nline2\n...line100")

# Test: Read all lines
read(contentId="xyz", fromLine=1, toLine=100)
# Expected: "End of file reached" (no truncation warning)

# Test: Read partial
read(contentId="xyz", fromLine=1, toLine=50)
# Expected: "... (truncated: 50 more lines remaining, total 100 lines)"
```

### Test Case 2: Line Count Visibility

```bash
# Test: List content
list()
# Expected output includes: "Size: 4441 bytes, 86 lines"
```

### Test Case 3: Out of Bounds Error

```bash
# Setup: File has 86 lines
# Test: Request invalid range
read(contentId="xyz", fromLine=5000, toLine=5300)
# Expected: "Error: Requested range [5000-5300] exceeds file length. File has 86 lines. Valid range: [1-86]"
```

---

## Summary

**Validated Issues:** 3 of 5 (60%)  
**Rejected Issues:** 1 (scratchpad contentId - already implemented)  
**Requires Clarification:** 1 (checkTodo verification - architectural)

**Recommended Prioritization:**

1. **Sprint 1 (P0):** Content store truncation + line count display (1-2 days)
2. **Sprint 2 (P1):** Error message refinement (1 day)
3. **Sprint 3 (P2):** System prompt updates (immediate, no code changes)
4. **Future:** Event log system for cross-tool verification (major feature)

**Next Steps:**

1. Implement Sprint 1 tasks
2. Run validation test cases
3. Monitor agent traces for improvement
4. Defer Sprint 3 (checkTodo verification) until event system design is complete

---

**Validation Completed:** 2026-01-10  
**Confidence Level:** High (direct code inspection performed)  
**Ready for Implementation:** Yes (P0/P1 tasks)
