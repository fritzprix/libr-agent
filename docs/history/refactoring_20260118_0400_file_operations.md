# Refactoring Plan: File Operations Enhancement

**Date:** 2026-01-18 04:00  
**Sprint:** W3-Jan-26  
**Branch:** dev/0.4.0  
**Scope:** File editing workflow improvements and tool discoverability

---

## ✅ Clarification Decisions (Approved)

**Q1: editLineInFile File Size Limit**

- **Decision:** Set 10,000 lines hard limit with clear error message
- **Rationale:** Files >10K lines exceed practical LLM context windows; prompt agents to split/chunk

**Q2: searchLineInFile Naming & Discoverability**

- **Decision:** Rename `grep` to `searchLineInFile`, maintain backward compatibility
- **Rationale:** Semantic clarity improves agent adoption; grep is technical jargon

**Q5: Tool Clarification - searchFiles vs searchLineInFile**

- **Decision:** Keep `searchLineInFile` (renamed from `grep`) for content search with line numbers; `searchFiles` remains for filename pattern matching
- **Rationale:** Clear separation of concerns - `searchFiles` uses glob patterns to find files by name (returns file paths), while `searchLineInFile` uses regex to search file content (returns line numbers)

---

## 작업의 목적 (Purpose)

### Primary Goals

1. **Prevent unnecessary file operations** - Detect and reject editFile calls with identical old/new values
2. **Improve tool discoverability** - Rename `grep` to `searchLineInFile` for better semantic clarity
3. **Streamline batch editing** - Introduce `editLineInFile` for atomic multi-line editing operations

### Expected Outcomes

- Reduced file I/O overhead from redundant edit operations
- Increased agent adoption of line-based search functionality
- Simplified workflow from 5+ editFile calls to single editLineInFile call for batch edits

---

## 현재의 상태 / 문제점 (Current State & Problems)

### Problem 1: Identical String Editing

**Current Behavior:**

```rust
// editFile currently allows:
editFile(path: "main.rs", oldString: "foo", newString: "foo")
// Result: Reads file, replaces nothing, writes file back (wasteful I/O)
```

**Impact:**

- Unnecessary disk writes wear out SSDs
- Misleading success message implies change occurred
- Agents don't learn from the mistake (no error feedback)

**Root Cause:** No pre-validation in `edit_replace.rs` before string matching phase

---

### Problem 2: Tool Discoverability ("grep" vs "searchLineInFile")

**Current Behavior:**

- Tool name: `grep`
- Agents don't naturally associate "grep" with "search by line number"
- Underutilized despite having all required functionality (regex, line numbers, exact match)

**Evidence:**

```rust
// grep tool in file_tools.rs
MCPTool {
    name: "grep".to_string(),  // ❌ Technical jargon
    description: "Search files with regex patterns..."
}
```

**Impact:**

- Agents resort to readFile + manual parsing instead of using grep
- Workflow becomes: readFile(entire file) → extract lines → process
- Should be: searchLineInFile(pattern) → get line numbers → editLineInFile

---

### Problem 3: Multi-Step Editing Workflow

**Current Workflow:**

```
Agent needs to edit 5 lines in same file:
1. searchLineInFile("pattern1") → line 10  // Content search returns line numbers
2. readFile("main.rs", 8, 12) → get context
3. editFile(old1, new1)
4. readFile("main.rs", 18, 22) → get context
5. editFile(old2, new2)
6. ... repeat 3 more times
```

**Problems:**

- 15+ tool calls for 5 line edits
- Each readFile needs updated line numbers after previous edits
- Risk of stale line numbers causing wrong-line edits
- No atomicity - partial edits leave inconsistent state

**Desired Workflow:**

```
1. searchLineInFile("pattern") → lines [10, 20, 30, 40, 50]  // Content search returns line numbers
2. editLineInFile([
     {line: 10, old_value: "foo", new_value: "bar"},
     {line: 20, old_value: "baz", new_value: "qux"},
     ...
   ])
```

---

## 관련 코드의 구조 및 동작 방식 Summary (Code Architecture)

### File Operations Module Structure

```
src-tauri/src/mcp/builtin/workspace/
├── file_operations/
│   ├── edit_replace.rs       ← editFile implementation
│   │   └── handle_edit_file()
│   │       ├── validate_path_with_error()
│   │       ├── read_file_as_string()
│   │       ├── count occurrences (old_string)
│   │       ├── replace_all()
│   │       ├── generate_diff()
│   │       └── write_file_atomic()
│   │
│   ├── search_query.rs       ← grep/searchLineInFile (content search) implementation
│   │   └── handle_grep()
│   │       ├── parse search parameters (mode: regex/exact)
│   │       ├── read_file_as_string()
│   │       ├── apply regex/exact match
│   │       ├── collect line numbers & context
│   │       └── format results
│   │
│   ├── read_write.rs         ← readFile/createFile
│   └── mod.rs                ← Module exports
│
└── tools/
    ├── file_tools.rs         ← MCPTool definitions
    │   ├── create_read_file_tool()
    │   ├── create_create_file_tool()
    │   ├── create_edit_file_tool()
    │   └── create_grep_tool()      ← To be renamed
    └── mod.rs

```

### Current editFile Flow

```rust
handle_edit_file(args, session_id)
├─ Layer 1: Parameter extraction
│  ├─ path: String
│  ├─ oldString: String (validated non-empty)
│  └─ newString: String (can be empty for deletion)
│
├─ Layer 2: Business logic
│  ├─ validate_path_with_error() → safe_path
│  ├─ read_file_as_string() → original_content
│  ├─ count occurrences of oldString
│  │  ├─ 0 matches → ErrorGuidance::ContentNotFound
│  │  ├─ 1 match → proceed
│  │  └─ 2+ matches → ErrorGuidance::AmbiguousOperation
│  │
│  ├─ replace_all() → new_content
│  ├─ generate_diff() → unified diff with ±3 context
│  └─ write_file_atomic() → persist changes
│
└─ Layer 3: Response
   └─ SuccessHint with diff output
```

### Current grep Flow

```rust
handle_grep(args, session_id)
├─ Parameters
│  ├─ path: String (file to search)
│  ├─ pattern: String (regex or exact match)
│  ├─ mode: "regex" | "exact" (default: "regex")
│  └─ context: u32 (lines of context, default: 0)
│
├─ Execution
│  ├─ validate_path_with_error()
│  ├─ read_file_as_string()
│  ├─ Apply search (regex::Regex or contains())
│  ├─ Collect matches with line numbers
│  └─ Format with context lines
│
└─ Response
   └─ Text output: "Line 42: matched content"
```

### Key Design Patterns

1. **ErrorGuidance Pattern**: All errors include actionable guidance for agents
2. **Layer Separation**: Parameter extraction → Business logic → Response formatting
3. **Atomic Operations**: File writes use temporary file + rename for atomicity
4. **Diff Generation**: Uses `similar` crate for unified diff output

---

## 변경 이후의 상태 / 해결 판정 기준 (Success Criteria)

### Task 1: editFile Identical String Validation

**Acceptance Criteria:**

- ✅ editFile rejects when `oldString == newString`
- ✅ Error message: "oldString and newString are identical - no changes needed"
- ✅ ErrorCategory: `InvalidInput` with guidance
- ✅ No file I/O performed (early exit before read operation)

**Validation Test:**

```rust
#[test]
async fn test_edit_file_identical_strings() {
    let result = workspace_server.handle_edit_file(json!({
        "path": "test.txt",
        "oldString": "hello world",
        "newString": "hello world"
    }), Some(session_id)).await;

    assert!(result.is_ok());
    let mcp_result = result.unwrap();
    assert_eq!(mcp_result.is_error, Some(true));
    assert!(mcp_result.content[0].text.contains("identical"));
}
```

---

### Task 2: grep → searchLineInFile Rename

**Acceptance Criteria:**

- ✅ Tool name changed from `grep` to `searchLineInFile`
- ✅ Tool description emphasizes line-based searching
- ✅ All existing functionality preserved (regex, exact match, context lines)
- ✅ Backward compatibility: old `grep` name still works (deprecated alias)

**Validation Test:**

```rust
#[test]
fn test_tool_registration() {
    let tools = file_tools();
    let search_tool = tools.iter()
        .find(|t| t.name == "searchLineInFile")
        .expect("searchLineInFile tool not found");

    assert!(search_tool.description.contains("line"));
    assert!(search_tool.description.contains("number"));
}
```

**Agent Behavior Verification:**

- Monitor agent logs for increased usage of searchLineInFile
- Measure reduction in readFile + manual parsing patterns

---

### Task 3: editLineInFile Tool Implementation

**Acceptance Criteria:**

- ✅ Accepts array of line edit operations: `[{line, old_value?, new_value}, ...]`
- ✅ Atomic operation: ALL edits succeed or ALL fail
- ✅ Conflict detection: Reject duplicate line numbers with clear error
- ✅ Single-line scope: Multi-line `new_value` rejected with guidance to use editFile
- ✅ Line number validation: All lines must exist in file
- ✅ Optional validation: `old_value` parameter validates before replacement
- ✅ Reverse order application: Edits applied high-to-low to preserve line stability
- ✅ Unified diff generation: Shows all changes in single diff output

**Validation Tests:**

```rust
#[test]
async fn test_edit_line_in_file_atomic() {
    // Test: One invalid edit causes entire operation to fail
    let result = workspace_server.handle_edit_line_in_file(json!({
        "path": "test.txt",
        "edits": [
            {"line": 10, "old_value": "valid", "new_value": "updated"},
            {"line": 999, "old_value": "invalid", "new_value": "fail"}  // Out of range
        ]
    }), Some(session_id)).await;

    assert!(result.is_ok());
    let mcp_result = result.unwrap();
    assert_eq!(mcp_result.is_error, Some(true));

    // Verify: File unchanged (atomicity)
    let content = read_file("test.txt").await.unwrap();
    assert!(content.contains("valid"));  // Not changed to "updated"
}

#[test]
async fn test_edit_line_in_file_conflict_detection() {
    // Test: Duplicate line numbers rejected
    let result = workspace_server.handle_edit_line_in_file(json!({
        "path": "test.txt",
        "edits": [
            {"line": 10, "new_value": "first"},
            {"line": 10, "new_value": "second"}  // Conflict!
        ]
    }), Some(session_id)).await;

    assert!(result.is_ok());
    let mcp_result = result.unwrap();
    assert_eq!(mcp_result.is_error, Some(true));
    assert!(mcp_result.content[0].text.contains("duplicate"));
}

#[test]
async fn test_edit_line_in_file_multiline_rejection() {
    // Test: Multi-line new_value rejected
    let result = workspace_server.handle_edit_line_in_file(json!({
        "path": "test.txt",
        "edits": [
            {"line": 10, "new_value": "line1\nline2"}  // Multi-line!
        ]
    }), Some(session_id)).await;

    assert!(result.is_ok());
    let mcp_result = result.unwrap();
    assert_eq!(mcp_result.is_error, Some(true));
    assert!(mcp_result.content[0].text.contains("single-line"));
    assert!(mcp_result.content[0].text.contains("editFile"));  // Guidance
}
```

---

## 수정이 필요한 코드 및 수정부분의 코드 스니핏 (Code Modifications)

### Modification 1: editFile Identical String Check

**File:** `src-tauri/src/mcp/builtin/workspace/file_operations/edit_replace.rs`  
**Location:** After parameter extraction, before path validation (around line 202)

```rust
// CURRENT CODE (lines 190-206):
        let new_string = match args.get("newString").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return Ok(missing_param_error("newString", ToolGroup::Workspace)),
        };

        // Layer 2: Business logic - path validation and file reading
        let safe_path = self.validate_path_with_error(path_str, session_id.clone())?;

// ADD THIS VALIDATION:
        // Validate: Reject identical strings early (before file I/O)
        if old_string == new_string {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                "oldString and newString are identical - no changes needed".to_string(),
                vec![
                    "Verify that oldString and newString are different".to_string(),
                    "If no changes needed, skip the editFile call".to_string(),
                    "Check for logic errors in value generation".to_string(),
                ],
                ToolGroup::Workspace,
            ).to_mcp_result());
        }

        // Layer 2: Business logic - path validation and file reading
        let safe_path = self.validate_path_with_error(path_str, session_id.clone())?;
```

**Rationale:**

- Early exit before expensive file I/O operations
- Prevents disk writes when no actual change occurs
- Provides clear feedback to agents about the issue

---

### Modification 2: Rename grep → searchLineInFile

**File:** `src-tauri/src/mcp/builtin/workspace/tools/file_tools.rs`  
**Function:** `create_grep_tool()` (around line 250)

```rust
// CURRENT CODE:
pub fn create_grep_tool() -> MCPTool {
    let mut props = HashMap::new();
    // ... properties definition ...

    MCPTool {
        name: "grep".to_string(),
        title: Some("Search Files".to_string()),
        description: "Search files with regex patterns or exact text matching...".to_string(),
        // ... rest of definition ...
    }
}

// MODIFIED CODE:
pub fn create_search_line_in_file_tool() -> MCPTool {  // ← Function renamed
    let mut props = HashMap::new();
    // ... properties definition (unchanged) ...

    MCPTool {
        name: "searchLineInFile".to_string(),  // ← Tool name changed
        title: Some("Search Lines in File".to_string()),  // ← Title updated
        description: "Search for text patterns in a file and get matching line numbers with context.

⚠️ PRIMARY USE CASE: Find line numbers for targeted editing
This tool returns line numbers where patterns match, enabling precise line-based edits.

SEARCH MODES:
- regex (default): Use regular expressions for pattern matching
- exact: Literal string matching (case-sensitive)

PARAMETERS:
- path: File to search (relative to workspace root)
- pattern: Search pattern (regex or exact string)
- mode: 'regex' or 'exact' (default: 'regex')
- context: Number of context lines to show (default: 0)

RETURNS:
- Line numbers of matches
- Matched content
- Optional context lines (±N lines around match)

💡 WORKFLOW:
1. Use searchLineInFile to find line numbers
2. Use editLineInFile for batch editing at those lines
3. Or use readFile + editFile for content-based editing

EXAMPLES:
- searchLineInFile({path: 'main.rs', pattern: 'fn handle_', mode: 'regex'})
  → Returns all lines with function definitions

- searchLineInFile({path: 'config.json', pattern: '\"debug\": true', mode: 'exact'})
  → Returns exact line number of debug config

✅ BEST FOR: Finding specific lines for editing within a single file
❌ NOT FOR: Finding files by name pattern (use searchFiles for glob-based file finding)

NOTE: This tool searches FILE CONTENT and returns LINE NUMBERS. For finding files by filename pattern, use searchFiles (which returns file paths).".to_string(),
        input_schema: object_schema(props, vec!["path".to_string(), "pattern".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
```

**File:** `src-tauri/src/mcp/builtin/workspace/tools/mod.rs`  
**Function:** `file_tools()` export list

```rust
// CURRENT CODE:
pub fn file_tools() -> Vec<MCPTool> {
    vec![
        create_read_file_tool(),
        create_create_file_tool(),
        create_edit_file_tool(),
        create_grep_tool(),  // ← Old name
        // ... other tools ...
    ]
}

// MODIFIED CODE:
pub fn file_tools() -> Vec<MCPTool> {
    vec![
        create_read_file_tool(),
        create_create_file_tool(),
        create_edit_file_tool(),
        create_search_line_in_file_tool(),  // ← New name
        // ... other tools ...
    ]
}
```

**File:** `src-tauri/src/mcp/builtin/workspace/mod.rs`  
**Function:** `call_tool()` routing logic

```rust
// CURRENT CODE:
pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
    match tool_name {
        "readFile" => self.handle_read_file(args, self.session_id.clone()).await,
        "grep" => self.handle_grep(args, self.session_id.clone()).await,  // ← Old name
        // ... other tools ...
    }
}

// MODIFIED CODE:
pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
    match tool_name {
        "readFile" => self.handle_read_file(args, self.session_id.clone()).await,
        "searchLineInFile" => self.handle_grep(args, self.session_id.clone()).await,  // ← New name
        "grep" => self.handle_grep(args, self.session_id.clone()).await,  // ← Backward compatibility
        // ... other tools ...
    }
}
```

**Rationale:**

- Semantic clarity: "searchLineInFile" clearly indicates line-based searching
- Maintains backward compatibility with deprecated "grep" alias
- Enhanced description guides agents toward line-editing workflow

---

### Modification 3: New editLineInFile Tool

**File:** `src-tauri/src/mcp/builtin/workspace/file_operations/edit_line.rs` (NEW FILE)

```rust
use super::super::WorkspaceServer;
use super::utils::{read_file_as_string, write_file_atomic};
use crate::mcp::builtin::error_guidance::*;
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use similar::{ChangeTag, TextDiff};
use std::collections::HashSet;

#[derive(Debug)]
struct LineEdit {
    line: usize,
    old_value: Option<String>,
    new_value: String,
}

impl WorkspaceServer {
    pub async fn handle_edit_line_in_file(
        &self,
        args: Value,
        session_id: Option<String>,
    ) -> Result<MCPResult, String> {
        // Layer 1: Parameter extraction
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) if !p.trim().is_empty() => p,
            _ => return Ok(missing_param_error("path", ToolGroup::Workspace)),
        };

        // Validate: Check file size limit (10MB)
        let safe_path = self.validate_path_with_error(path_str, session_id.clone())?;

        if let Ok(metadata) = tokio::fs::metadata(&safe_path).await {
            const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB
            if metadata.len() > MAX_FILE_SIZE {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!(
                        "File size ({:.2}MB) exceeds 10MB limit",
                        metadata.len() as f64 / (1024.0 * 1024.0)
                    ),
                    vec![
                        "Files larger than 10MB exceed typical LLM context windows".to_string(),
                        "Consider splitting the file into smaller chunks".to_string(),
                        "Or use selective editing with readFile + editFile for specific sections".to_string(),
                    ],
                    ToolGroup::Workspace,
                ).to_mcp_result());
            }
        }

        let edits_array = match args.get("edits").and_then(|v| v.as_array()) {
            Some(arr) if !arr.is_empty() => arr,
            _ => return Ok(missing_param_error("edits", ToolGroup::Workspace)),
        };

        // Validate: Check line count limit (10,000 lines) - Decision from Q1
        let safe_path = self.validate_path_with_error(path_str, session_id.clone())?;

        // Read file to count lines (needed anyway for line-based editing)
        let original_content = match read_file_as_string(&safe_path).await {
            Ok(content) => content,
            Err(e) => {
                return Ok(operation_failed_error(
                    "Read file for line editing",
                    &e,
                    vec![
                        "Verify the file exists with listDirectory".to_string(),
                        "Check file permissions".to_string(),
                    ],
                    ToolGroup::Workspace,
                ));
            }
        };

        let lines: Vec<&str> = original_content.lines().collect();
        let total_lines = lines.len();
        const MAX_LINES: usize = 10_000;

        if total_lines > MAX_LINES {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::InvalidInput,
                format!(
                    "File has {} lines, exceeds {MAX_LINES} line limit",
                    total_lines
                ),
                vec![
                    "Files larger than 10,000 lines exceed practical LLM context windows".to_string(),
                    "Consider splitting the file into smaller files".to_string(),
                    "Or use selective editing with readFile + editFile for specific sections".to_string(),
                ],
                ToolGroup::Workspace,
            ).to_mcp_result());
        }

        // Parse and validate edit operations
        let mut edits = Vec::new();
        let mut line_numbers = HashSet::new();

        for (idx, edit_obj) in edits_array.iter().enumerate() {
            let line = match edit_obj.get("line").and_then(|v| v.as_u64()) {
                Some(l) if l > 0 => l as usize,
                _ => {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidInput,
                        format!("Edit #{}: 'line' must be a positive integer", idx + 1),
                        vec![
                            "Line numbers are 1-based (first line is 1)".to_string(),
                            "Use searchLineInFile to get correct line numbers".to_string(),
                        ],
                        ToolGroup::Workspace,
                    ).to_mcp_result());
                }
            };

            // Conflict detection: Check for duplicate line numbers
            if !line_numbers.insert(line) {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!("Duplicate line number detected: line {}", line),
                    vec![
                        "Each line can only be edited once per editLineInFile call".to_string(),
                        "Combine multiple edits to same line into single edit".to_string(),
                        "Or use multiple editLineInFile calls sequentially".to_string(),
                    ],
                    ToolGroup::Workspace,
                ).to_mcp_result());
            }

            let new_value = match edit_obj.get("new_value").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::InvalidInput,
                        format!("Edit #{}: 'new_value' is required", idx + 1),
                        vec![
                            "Provide the new content for this line".to_string(),
                            "Use empty string \"\" to delete line content".to_string(),
                        ],
                        ToolGroup::Workspace,
                    ).to_mcp_result());
                }
            };

            // Multi-line validation: Reject if new_value contains newlines
            if new_value.contains('\n') {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!("Edit #{}: 'new_value' contains newline characters", idx + 1),
                    vec![
                        "editLineInFile only supports single-line replacements".to_string(),
                        "For multi-line replacements, use editFile instead".to_string(),
                        "Workflow: readFile → extract context → editFile(old_text, new_text)".to_string(),
                    ],
                    ToolGroup::Workspace,
                ).to_mcp_result());
            }

            let old_value = edit_obj.get("old_value").and_then(|v| v.as_str()).map(|s| s.to_string());

            edits.push(LineEdit {
                line,
                old_value,
                new_value,
            });
        }

        // Layer 2: Business logic - read file and validate
        let safe_path = self.validate_path_with_error(path_str, session_id.clone())?;

        let original_content = match read_file_as_string(&safe_path).await {
            Ok(content) => content,
            Err(e) => {
                return Ok(operation_failed_error(
                    "Read file for line editing",
                    &e,
                    vec![
                        "Verify the file exists with listDirectory".to_string(),
                        "Check file permissions".to_string(),
                    ],
                    ToolGroup::Workspace,
                ));
            }
        };

        let lines: Vec<&str> = original_content.lines().collect();
        let total_lines = lines.len();

        // Validate all line numbers are within range
        for edit in &edits {
            if edit.line > total_lines {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::NotFound,
                    format!("Line {} does not exist (file has {} lines)", edit.line, total_lines),
                    vec![
                        "Use searchLineInFile to verify line numbers".to_string(),
                        "Line numbers may change after previous edits".to_string(),
                        "Use readFile to see current file content and line count".to_string(),
                    ],
                    ToolGroup::Workspace,
                ).to_mcp_result());
            }

            // Validate old_value if provided (content verification)
            if let Some(ref expected_old) = edit.old_value {
                let actual_line = lines[edit.line - 1];  // Convert to 0-based
                if actual_line != expected_old {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::ContentNotFound,
                        format!(
                            "Line {} content mismatch:\nExpected: \"{}\"\nActual: \"{}\"",
                            edit.line, expected_old, actual_line
                        ),
                        vec![
                            "The file content has changed since you retrieved line numbers".to_string(),
                            "Use readFile to get current content".to_string(),
                            "Then retry with updated old_value or without old_value validation".to_string(),
                        ],
                        ToolGroup::Workspace,
                    ).to_mcp_result());
                }
            }
        }

        // Sort edits by line number (descending) for stable application
        let mut sorted_edits = edits.clone();
        sorted_edits.sort_by(|a, b| b.line.cmp(&a.line));

        // Apply edits in reverse order (high line to low line)
        let mut modified_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        for edit in sorted_edits {
            modified_lines[edit.line - 1] = edit.new_value.clone();
        }

        let new_content = modified_lines.join("\n");
        if original_content.ends_with('\n') && !new_content.ends_with('\n') {
            new_content.push('\n');
        }

        // Generate unified diff
        // Note: Context lines configurable via Settings page (default: 3)
        // Agents can read current setting with readFile if needed
        let context_lines = 3; // TODO: Read from app settings

        let diff = TextDiff::from_lines(&original_content, &new_content);
        let mut diff_output = String::new();
        diff_output.push_str(&format!("--- {}\n", path_str));
        diff_output.push_str(&format!("+++ {}\n", path_str));

        for (idx, group) in diff.grouped_ops(context_lines).iter().enumerate() {
            if idx > 0 {
                diff_output.push_str("---\n");
            }
            for op in group {
                for change in diff.iter_inline_changes(op) {
                    let (sign, style) = match change.tag() {
                        ChangeTag::Delete => ("-", ""),
                        ChangeTag::Insert => ("+", ""),
                        ChangeTag::Equal => (" ", ""),
                    };
                    diff_output.push_str(&format!("{} {}", sign, change));
                }
            }
        }

        // Write file atomically
        if let Err(e) = write_file_atomic(&safe_path, new_content.as_bytes()).await {
            return Ok(operation_failed_error(
                "Write file after line edits",
                &e,
                vec![
                    "Check file permissions".to_string(),
                    "Verify disk space is available".to_string(),
                ],
                ToolGroup::Workspace,
            ));
        }

        // Layer 3: Success response
        Ok(SuccessHint::new(
            format!(
                "Successfully edited {} line(s) in {}\n\nDiff:\n{}",
                edits.len(),
                path_str,
                diff_output
            ),
            vec![
                "All line edits applied atomically".to_string(),
                "Use readFile to verify changes".to_string(),
            ],
        )
        .to_mcp_result())
    }
}
```

**File:** `src-tauri/src/mcp/builtin/workspace/tools/file_tools.rs` (ADD NEW FUNCTION)

```rust
pub fn create_edit_line_in_file_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path to the file to edit (from workspace root)"),
        ),
    );

    props.insert(
        "edits".to_string(),
        json_schema!(
            "Array of line edit operations to apply atomically",
            {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["line", "new_value"],
                    "properties": {
                        "line": {
                            "type": "integer",
                            "minimum": 1,
                            "description": "Line number to edit (1-based)"
                        },
                        "old_value": {
                            "type": "string",
                            "description": "Optional: Expected current value for validation"
                        },
                        "new_value": {
                            "type": "string",
                            "description": "New content for this line (single-line only)"
                        }
                    }
                },
                "minItems": 1
            }
        ),
    );

    MCPTool {
        name: "editLineInFile".to_string(),
        title: Some("Edit Multiple Lines".to_string()),
        description: "Edit multiple lines in a file atomically. All edits succeed or all fail.

⚠️ CRITICAL: ATOMIC OPERATION
- ALL line edits must be valid or ENTIRE operation fails
- No partial edits - file remains unchanged if any edit fails
- Use for batch single-line edits (5+ edits recommended)
- Line count limit: 10,000 lines maximum (exceeds practical LLM context)

WORKFLOW:
1. Use searchLineInFile to find line numbers
2. Use editLineInFile to edit multiple lines at once
3. All changes applied together or none at all

PARAMETERS:
- path: File to edit (relative to workspace root)
- edits: Array of {line, old_value?, new_value}
  - line: 1-based line number (required)
  - old_value: Expected content for validation (optional but recommended)
  - new_value: New line content (required, single-line only)

VALIDATION:
✅ All line numbers must exist in file
✅ No duplicate line numbers allowed (conflict detection)
✅ If old_value provided, must match current content
✅ new_value must be single-line (no \\n characters)

EXAMPLE:
{
  \"path\": \"src/main.rs\",
  \"edits\": [
    {\"line\": 10, \"old_value\": \"let x = 1;\", \"new_value\": \"let x = 2;\"},
    {\"line\": 20, \"old_value\": \"let y = 3;\", \"new_value\": \"let y = 4;\"},
    {\"line\": 30, \"new_value\": \"// New comment\"}
  ]
}

ERROR HANDLING:
❌ Line out of range → FAIL (no changes applied)
❌ Duplicate line number → FAIL (no changes applied)
❌ old_value mismatch → FAIL (no changes applied)
❌ Multi-line new_value → FAIL with guidance to use editFile

💡 WHEN TO USE:
✅ Editing 5+ lines in same file
✅ Batch updates with known line numbers
✅ Replacing similar patterns across multiple lines

❌ WHEN NOT TO USE:
- Multi-line replacements → use editFile instead
- Unknown line numbers → use searchLineInFile first
- Content-based editing → use readFile + editFile

🔗 RELATED TOOLS:
- searchLineInFile: Find line numbers for editing
- readFile: Get current file content and verify changes
- editFile: For content-based or multi-line edits".to_string(),
        input_schema: object_schema(props, vec!["path".to_string(), "edits".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
```

**File:** `src-tauri/src/mcp/builtin/workspace/tools/mod.rs` (UPDATE EXPORTS)

```rust
pub fn file_tools() -> Vec<MCPTool> {
    vec![
        create_read_file_tool(),
        create_create_file_tool(),
        create_edit_file_tool(),
        create_edit_line_in_file_tool(),  // ← ADD NEW TOOL
        create_search_line_in_file_tool(),
        // ... other tools ...
    ]
}
```

**File:** `src-tauri/src/mcp/builtin/workspace/mod.rs` (ADD ROUTING)

```rust
pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String> {
    match tool_name {
        "readFile" => self.handle_read_file(args, self.session_id.clone()).await,
        "editFile" => self.handle_edit_file(args, self.session_id.clone()).await,
        "editLineInFile" => self.handle_edit_line_in_file(args, self.session_id.clone()).await,  // ← ADD
        "searchLineInFile" => self.handle_grep(args, self.session_id.clone()).await,
        // ... other tools ...
    }
}
```

---

## 재사용 가능한 연관 코드 (Reusable Related Code)

### Utility Functions (Already Implemented)

**File:** `src-tauri/src/mcp/builtin/workspace/file_operations/utils.rs`

```rust
/// Read entire file as UTF-8 string
pub async fn read_file_as_string(path: &Path) -> Result<String, std::io::Error> {
    tokio::fs::read_to_string(path).await
}

/// Write file atomically using temp file + rename
pub async fn write_file_atomic(path: &Path, content: &[u8]) -> Result<(), std::io::Error> {
    let temp_path = path.with_extension("tmp");
    tokio::fs::write(&temp_path, content).await?;
    tokio::fs::rename(&temp_path, path).await?;
    Ok(())
}
```

**Interface:** Simple async file I/O with error handling  
**Reusability:** Used by editFile, editLineInFile, and all file operation tools

---

### Error Guidance System

**File:** `src-tauri/src/mcp/builtin/error_guidance.rs`

```rust
pub struct ErrorGuidance {
    pub category: ErrorCategory,
    pub message: String,
    pub guidance: Vec<String>,
    pub tool_group: ToolGroup,
}

impl ErrorGuidance {
    pub fn with_guidance(
        category: ErrorCategory,
        message: String,
        guidance: Vec<String>,
        tool_group: ToolGroup,
    ) -> Self { ... }

    pub fn to_mcp_result(&self) -> MCPResult { ... }
}

pub struct SuccessHint {
    pub message: String,
    pub next_steps: Vec<String>,
}

impl SuccessHint {
    pub fn new(message: String, next_steps: Vec<String>) -> Self { ... }
    pub fn to_mcp_result(&self) -> MCPResult { ... }
}
```

**Interface:** Standardized error and success responses with actionable guidance  
**Reusability:** All tools use this for consistent agent feedback

---

### Diff Generation (similar crate)

**File:** External dependency in `Cargo.toml`

```toml
[dependencies]
similar = "2.3"  # Text diffing library
```

**Usage Example:**

```rust
use similar::{ChangeTag, TextDiff};

let diff = TextDiff::from_lines(&original, &modified);
for group in diff.grouped_ops(3) {  // 3 lines of context
    for op in group {
        for change in diff.iter_inline_changes(op) {
            match change.tag() {
                ChangeTag::Delete => print!("-"),
                ChangeTag::Insert => print!("+"),
                ChangeTag::Equal => print!(" "),
            }
            print!("{}", change);
        }
    }
}
```

**Interface:** Unified diff generation with configurable context lines  
**Reusability:** Used by editFile and editLineInFile for visual change feedback

---

### Path Validation

**File:** `src-tauri/src/mcp/builtin/workspace/mod.rs`

```rust
impl WorkspaceServer {
    pub fn validate_path_with_error(
        &self,
        path_str: &str,
        session_id: Option<String>,
    ) -> Result<PathBuf, String> {
        // Validates:
        // 1. No path traversal (../)
        // 2. Within workspace boundaries
        // 3. Session-specific workspace isolation
        // 4. No absolute paths outside workspace
    }
}
```

**Interface:** Secure path validation for all file operations  
**Reusability:** All file operation tools use this for security

---

## Test Code 추가 및 수정 필요 부분에 대한 가이드 (Test Guidelines)

### Test File Structure

```
src-tauri/tests/
├── workspace_tests.rs           ← Existing integration tests
└── file_operations_tests.rs     ← NEW: Dedicated file operation tests
```

### Test Cases for Task 1: editFile Identical String

**File:** `src-tauri/tests/file_operations_tests.rs` (NEW)

```rust
#[tokio::test]
async fn test_edit_file_identical_strings_rejected() {
    let workspace = setup_test_workspace().await;
    let session_id = create_test_session(&workspace).await;

    // Create test file
    write_test_file(&workspace, "test.txt", "hello world\n").await;

    // Attempt edit with identical strings
    let result = workspace.handle_edit_file(
        json!({
            "path": "test.txt",
            "oldString": "hello world",
            "newString": "hello world"
        }),
        Some(session_id.clone())
    ).await;

    // Assertions
    assert!(result.is_ok());
    let mcp_result = result.unwrap();
    assert_eq!(mcp_result.is_error, Some(true));
    assert!(mcp_result.content[0].text.contains("identical"));
    assert!(mcp_result.content[0].text.contains("no changes needed"));

    // Verify file unchanged
    let content = read_test_file(&workspace, "test.txt").await;
    assert_eq!(content, "hello world\n");
}

#[tokio::test]
async fn test_edit_file_identical_strings_no_io() {
    // Test that no file I/O occurs (performance test)
    let workspace = setup_test_workspace().await;
    let session_id = create_test_session(&workspace).await;

    // Create test file
    write_test_file(&workspace, "large.txt", &"x".repeat(1_000_000)).await;

    let start = std::time::Instant::now();

    // Should fail fast without reading 1MB file
    let result = workspace.handle_edit_file(
        json!({
            "path": "large.txt",
            "oldString": "identical",
            "newString": "identical"
        }),
        Some(session_id)
    ).await;

    let elapsed = start.elapsed();

    assert!(result.is_ok());
    assert_eq!(result.unwrap().is_error, Some(true));
    assert!(elapsed.as_millis() < 10);  // Should be instant (no file read)
}
```

---

### Test Cases for Task 2: searchLineInFile Rename

**File:** `src-tauri/tests/file_operations_tests.rs`

```rust
#[tokio::test]
async fn test_search_line_in_file_tool_exists() {
    let tools = file_tools();
    let search_tool = tools.iter()
        .find(|t| t.name == "searchLineInFile")
        .expect("searchLineInFile tool not found");

    assert!(search_tool.description.contains("line"));
    assert!(search_tool.description.contains("number"));
    assert!(search_tool.description.contains("editLineInFile"));  // Workflow guidance
}

#[tokio::test]
async fn test_search_line_in_file_backward_compatibility() {
    let workspace = setup_test_workspace().await;
    let session_id = create_test_session(&workspace).await;

    write_test_file(&workspace, "test.txt", "foo\nbar\nbaz\n").await;

    // Test new name
    let result1 = workspace.call_tool("searchLineInFile", json!({
        "path": "test.txt",
        "pattern": "bar",
        "mode": "exact"
    })).await;
    assert!(result1.is_ok());

    // Test old name (deprecated but still works)
    let result2 = workspace.call_tool("grep", json!({
        "path": "test.txt",
        "pattern": "bar",
        "mode": "exact"
    })).await;
    assert!(result2.is_ok());

    // Results should be identical
    assert_eq!(
        result1.unwrap().content[0].text,
        result2.unwrap().content[0].text
    );
}
```

---

### Test Cases for Task 3: editLineInFile

**File:** `src-tauri/tests/file_operations_tests.rs`

```rust
#[tokio::test]
async fn test_edit_line_in_file_atomic_success() {
    let workspace = setup_test_workspace().await;
    let session_id = create_test_session(&workspace).await;

    write_test_file(&workspace, "test.txt", "line1\nline2\nline3\nline4\nline5\n").await;

    let result = workspace.handle_edit_line_in_file(
        json!({
            "path": "test.txt",
            "edits": [
                {"line": 2, "old_value": "line2", "new_value": "MODIFIED2"},
                {"line": 4, "old_value": "line4", "new_value": "MODIFIED4"}
            ]
        }),
        Some(session_id)
    ).await;

    assert!(result.is_ok());
    let mcp_result = result.unwrap();
    assert_eq!(mcp_result.is_error, Some(false));

    let content = read_test_file(&workspace, "test.txt").await;
    assert_eq!(content, "line1\nMODIFIED2\nline3\nMODIFIED4\nline5\n");
}

#[tokio::test]
async fn test_edit_line_in_file_atomic_failure_rollback() {
    let workspace = setup_test_workspace().await;
    let session_id = create_test_session(&workspace).await;

    write_test_file(&workspace, "test.txt", "line1\nline2\nline3\n").await;

    // One valid edit, one invalid (line out of range)
    let result = workspace.handle_edit_line_in_file(
        json!({
            "path": "test.txt",
            "edits": [
                {"line": 2, "new_value": "MODIFIED2"},
                {"line": 999, "new_value": "INVALID"}  // Out of range!
            ]
        }),
        Some(session_id)
    ).await;

    assert!(result.is_ok());
    let mcp_result = result.unwrap();
    assert_eq!(mcp_result.is_error, Some(true));
    assert!(mcp_result.content[0].text.contains("does not exist"));

    // CRITICAL: Verify file UNCHANGED (atomicity)
    let content = read_test_file(&workspace, "test.txt").await;
    assert_eq!(content, "line1\nline2\nline3\n");  // Still original
}

#[tokio::test]
async fn test_edit_line_in_file_conflict_detection() {
    let workspace = setup_test_workspace().await;
    let session_id = create_test_session(&workspace).await;

    write_test_file(&workspace, "test.txt", "line1\nline2\nline3\n").await;

    // Duplicate line numbers
    let result = workspace.handle_edit_line_in_file(
        json!({
            "path": "test.txt",
            "edits": [
                {"line": 2, "new_value": "FIRST"},
                {"line": 2, "new_value": "SECOND"}  // Conflict!
            ]
        }),
        Some(session_id)
    ).await;

    assert!(result.is_ok());
    let mcp_result = result.unwrap();
    assert_eq!(mcp_result.is_error, Some(true));
    assert!(mcp_result.content[0].text.contains("Duplicate line number"));
}

#[tokio::test]
async fn test_edit_line_in_file_multiline_rejection() {
    let workspace = setup_test_workspace().await;
    let session_id = create_test_session(&workspace).await;

    write_test_file(&workspace, "test.txt", "line1\nline2\n").await;

    // new_value contains newline
    let result = workspace.handle_edit_line_in_file(
        json!({
            "path": "test.txt",
            "edits": [
                {"line": 1, "new_value": "multi\nline"}  // Invalid!
            ]
        }),
        Some(session_id)
    ).await;

    assert!(result.is_ok());
    let mcp_result = result.unwrap();
    assert_eq!(mcp_result.is_error, Some(true));
    assert!(mcp_result.content[0].text.contains("single-line"));
    assert!(mcp_result.content[0].text.contains("editFile"));  // Guidance
}

#[tokio::test]
async fn test_edit_line_in_file_validation_with_old_value() {
    let workspace = setup_test_workspace().await;
    let session_id = create_test_session(&workspace).await;

    write_test_file(&workspace, "test.txt", "original\n").await;

    // old_value mismatch
    let result = workspace.handle_edit_line_in_file(
        json!({
            "path": "test.txt",
            "edits": [
                {"line": 1, "old_value": "wrong", "new_value": "new"}
            ]
        }),
        Some(session_id)
    ).await;

    assert!(result.is_ok());
    let mcp_result = result.unwrap();
    assert_eq!(mcp_result.is_error, Some(true));
    assert!(mcp_result.content[0].text.contains("mismatch"));
    assert!(mcp_result.content[0].text.contains("Expected: \"wrong\""));
    assert!(mcp_result.content[0].text.contains("Actual: \"original\""));
}

#[tokio::test]
async fn test_edit_line_in_file_reverse_order_stability() {
    let workspace = setup_test_workspace().await;
    let session_id = create_test_session(&workspace).await;

    write_test_file(&workspace, "test.txt", "1\n2\n3\n4\n5\n").await;

    // Edit lines in random order (should be applied high-to-low)
    let result = workspace.handle_edit_line_in_file(
        json!({
            "path": "test.txt",
            "edits": [
                {"line": 1, "new_value": "A"},  // Low line
                {"line": 5, "new_value": "E"},  // High line
                {"line": 3, "new_value": "C"}   // Middle line
            ]
        }),
        Some(session_id)
    ).await;

    assert!(result.is_ok());
    let content = read_test_file(&workspace, "test.txt").await;
    assert_eq!(content, "A\n2\nC\n4\nE\n");
}
```

### Test Execution Commands

```bash
# Run all file operation tests
cargo test --test file_operations_tests

# Run specific test
cargo test --test file_operations_tests test_edit_file_identical_strings

# Run with output
cargo test --test file_operations_tests -- --nocapture

# Run integration tests (includes workspace tests)
cargo test --tests
```

---

## Clarification Q-list

### Q1: editLineInFile - File Size Limit

**Question:** Should editLineInFile support large files with optimized memory usage?

**Context:** Current implementation reads entire file into memory. For large files (>10MB), this may cause memory pressure.

**Options:**

- A) Keep simple in-memory implementation (easier to maintain)
- B) Add streaming for files >10MB (more complex)
- C) Set hard limit (reject files >10MB)

**Answer:** Set limit to 10,000 lines maximum. If the file exceeds 10K lines, it doesn't fit into practical LLM context windows, so there is no use case where the LLM should edit such a large file directly. Return error message clearly acknowledging the line count limit and prompt the agent to split or chunk the file.

---

### Q2: Tool Naming & Discoverability

**Question:** How important is the rename from "grep" to "searchLineInFile"?

**Answer:** Semantic clarity improves agent adoption. "searchLineInFile" clearly indicates line-based searching, while "grep" is technical jargon.

---

### Q5: Tool Clarification - searchFiles vs searchLineInFile

**Question:** What's the relationship between `searchFiles` and `searchLineInFile`?

**Answer:** These are two distinct tools with different purposes:

- **`searchLineInFile`** (renamed from `grep`): Searches **file content** using regex patterns, returns **line numbers** with matched text. Single-file operation.
- **`searchFiles`** (not yet implemented): Searches **filesystem** using glob patterns, returns **file paths** matching filename patterns. Multi-file operation.

**Key Distinction:** Line numbers only make sense for content search (`searchLineInFile`), not filename search (`searchFiles` returns paths, not line numbers).

---

## 추가 분석 과제 (Additional Analysis Tasks)

### Task 1: Agent Usage Pattern Analysis

**Purpose:** Understand how agents currently use file editing tools

**Analysis Steps:**

1. Query agent logs for editFile usage patterns
2. Identify common multi-edit workflows
3. Measure average file sizes being edited
4. Count grep tool usage vs readFile+manual parsing

---

### Task 2: Cross-Platform File Operations

**Purpose:** Verify file operations work reliably across platforms

**Testing Required:**

- Test on Windows, macOS, Linux
- Test with various file encodings
- Test with large files near 10K line limit

---

**Document Status:** Ready for File Operations Implementation  
**Phase 1 Priority:** High (Quick wins)  
**Estimated Completion:** 4-5 hours
