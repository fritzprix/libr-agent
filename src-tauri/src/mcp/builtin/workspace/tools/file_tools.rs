use crate::mcp::{utils::schema_builder::*, MCPTool};

use std::collections::HashMap;

// Note: maximum file size is enforced at runtime (LIBRAGENT_MAX_FILE_SIZE).
// The input schema cannot call runtime functions; therefore `content` has no hard cap here.

pub fn create_read_file_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path to the file to read (from workspace root)"),
        ),
    );
    props.insert(
        "startLine".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("Starting line number (1-based, optional)"),
        ),
    );
    props.insert(
        "endLine".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("Ending line number (1-based, optional)"),
        ),
    );

    MCPTool {
        name: "readFile".to_string(),
        title: Some("Read File".to_string()),
        description: "Read the contents of a file from the workspace. Returns file content as text.

⚠️ CRITICAL WORKFLOW (PREREQUISITE FOR EDITS):
1. ALWAYS call readFile BEFORE editFile or previewReplacement
2. Extract exact text from readFile response (including whitespace)
3. Use extracted text as oldString parameter in editFile
4. Verify file exists with listDirectory if needed

PARAMETERS:
- path: Relative path from workspace root
- startLine (optional): Read from this line number (1-based)
- endLine (optional): Read up to this line number (1-based)

USAGE:
- Use readFile(path) to read entire file
- Use readFile(path, startLine, endLine) to read specific line ranges
- Line ranges are inclusive [startLine, endLine]

❌ NEVER edit files without reading them first
✅ ALWAYS use readFile before any edit operation

💡 NEXT: Use editFile for targeted changes or createFile for new files"
            .to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_create_file_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some(
                "Relative path from workspace root. Examples: 'src/main.rs', 'docs/README.md'

⚠️ VALIDATION:
- Must be relative path (no '../' traversal)
- Must not exist (checked before creation)
- Parent directories created automatically

💡 TIP: Use listDirectory('.') to see current workspace structure",
            ),
        ),
    );
    props.insert(
        "content".to_string(),
        string_prop(
            None,
            None,
            Some("Content to write to the new file. Maximum size enforced server-side via LIBRAGENT_MAX_FILE_SIZE

⚠️ SIZE LIMITS:
- Typical limit: 10MB per file
- For larger files: Split into smaller files or use external storage

💡 TIP: Empty content is allowed for creating placeholder files"),
        ),
    );

    MCPTool {
        name: "createFile".to_string(),
        title: Some("Create File".to_string()),
        description: "Create a new file in the workspace. FAILS if file already exists.

⚠️ CRITICAL BEHAVIOR:
- Creates NEW files only
- FAILS if file already exists (prevents accidental overwrites)
- Returns error with guidance for existing files

💡 WORKFLOW:
1. Check if file exists: Use listDirectory or readFile first
2. For new files: Call createFile directly
3. For existing files: Use editFile for incremental changes
4. For complete replacement: deleteFile → createFile

⚠️ COMMON SCENARIOS:
- New file: createFile(path, content) → Success
- Existing file (modify): Use editFile instead
- Existing file (replace all): deleteFile(path) → createFile(path, content)

💡 NEXT: Use readFile to verify content or listDirectory to see workspace structure"
            .to_string(),
        input_schema: object_schema(props, vec!["path".to_string(), "content".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_list_directory_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path to the directory to list (from workspace root)"),
        ),
    );

    MCPTool {
        name: "listDirectory".to_string(),
        title: Some("List Directory".to_string()),
        description: "List all files and subdirectories in a workspace directory. Returns names and types (file/directory).

⚠️ CRITICAL WORKFLOW (ENTRY POINT):
1. Start with listDirectory('.') to see workspace root
2. Identify target files or subdirectories
3. Use readFile(path) to view file contents
4. Use listDirectory(subdir) to explore subdirectories

USAGE:
- Use listDirectory('.') to see workspace root contents
- Use listDirectory('src') to explore subdirectories
- Navigate deeper by concatenating paths: 'src/components'

✅ ALWAYS start exploration with listDirectory
💡 TIP: Use searchFiles for finding specific file patterns

💡 NEXT: Use readFile to examine files or listDirectory to explore deeper".to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_import_file_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "srcAbsPath".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Absolute path of source file to import"),
        ),
    );
    props.insert(
        "destRelPath".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path in workspace where file will be imported"),
        ),
    );

    MCPTool {
        name: "importFile".to_string(),
        title: Some("Import File".to_string()),
        description: "Import an external file into the workspace".to_string(),
        input_schema: object_schema(
            props,
            vec!["srcAbsPath".to_string(), "destRelPath".to_string()],
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_edit_file_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path to the file to modify (from workspace root)"),
        ),
    );
    props.insert(
        "oldString".to_string(),
        string_prop(
            None,
            None,
            Some("⚠️ CRITICAL: Exact text content to find and replace. Must match precisely including whitespace.

MANDATORY WORKFLOW:
1. Call readFile(path) FIRST to get current content
2. Extract the exact text from readFile response (including all whitespace)
3. Include surrounding context (3-5 lines) for uniqueness
4. Use the extracted text as this parameter

❌ NEVER use text reconstructed from previous attempts or assumed values
✅ ALWAYS use text exactly as returned by readFile to ensure exact match

💡 TIP: For multiple changes, call this tool multiple times sequentially"),
        ),
    );
    props.insert(
        "newString".to_string(),
        string_prop(
            None,
            None,
            Some("New text content to replace oldString with. Use empty string to delete the matched text."),
        ),
    );

    MCPTool {
        name: "editFile".to_string(),
        title: Some("Edit File".to_string()),
        description: "Replace text content in a file using exact string matching. Atomic operation - either succeeds completely or fails with clear guidance.

⚠️ CRITICAL WORKFLOW (MUST FOLLOW):
1. ALWAYS call readFile(path) or readFile(path, startLine, endLine) FIRST
2. Extract the exact text from readFile response into oldString parameter
3. Verify the extracted text includes surrounding context (3-5 lines) for uniqueness
4. Then call editFile with the extracted oldString

💡 MULTIPLE CHANGES: Call this tool multiple times sequentially
   → Each call is atomic and independent
   → Easier to track and debug than batch operations
   → File state is consistent between calls

❌ NEVER use oldString reconstructed from previous attempts or assumed values
✅ ALWAYS use text exactly as returned by readFile to ensure exact match

⚠️ ERROR RECOVERY:
- If 'Pattern not found' error: Call readFile again to get updated content
- If 'Multiple matches' error: Include more surrounding context (5-10 lines)
- If 'File changed' error: Re-read file before retrying
- DO NOT retry with same oldString after failure

💡 NEXT: Use previewReplacement to verify changes before committing or readFile to confirm edits".to_string(),
        input_schema: object_schema(
            props,
            vec![
                "path".to_string(),
                "oldString".to_string(),
                "newString".to_string(),
            ],
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_preview_replacement_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path to the file (from workspace root)"),
        ),
    );
    props.insert(
        "oldString".to_string(),
        string_prop(
            None,
            None,
            Some("Text content you want to find and replace. Extract from readFile response."),
        ),
    );
    props.insert(
        "newString".to_string(),
        string_prop(
            None,
            None,
            Some("New text content to replace oldString with."),
        ),
    );

    MCPTool {
        name: "previewReplacement".to_string(),
        title: Some("Preview File Replacement".to_string()),
        description: "Preview what would change if editFile is executed. Shows exact diffs without modifying the file.

🎯 USE CASE: Verify oldString matches before committing changes

WORKFLOW:
1. Call readFile(path) to get current content
2. Call previewReplacement(path, oldString, newString) to see what would change
3. Review the diff output (shows ± lines with context)
4. If preview looks correct, call editFile with SAME parameters

✅ BENEFITS:
- Catch mismatches early without file corruption
- See exact line numbers and context
- Verify oldString was extracted correctly from readFile

⚠️ READ-ONLY: This tool does NOT modify files, only shows preview".to_string(),
        input_schema: object_schema(
            props,
            vec![
                "path".to_string(),
                "oldString".to_string(),
                "newString".to_string(),
            ],
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_search_line_in_file_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path to the file to search (from workspace root)"),
        ),
    );
    props.insert(
        "pattern".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Search pattern (regex or exact string, depending on mode)"),
        ),
    );
    props.insert(
        "mode".to_string(),
        string_prop(
            None,
            None,
            Some("Search mode: 'regex' (default) or 'exact'"),
        ),
    );
    props.insert(
        "ignoreCase".to_string(),
        boolean_prop(Some("Case-insensitive search (default: false)")),
    );
    props.insert(
        "lineNumbers".to_string(),
        boolean_prop(Some("Include line numbers in results (default: true)")),
    );

    MCPTool {
        name: "searchLineInFile".to_string(),
        title: Some("Search Lines in File".to_string()),
        description: "Search for text patterns in a file and get matching line numbers with context.

⚠️ PRIMARY USE CASE: Find line numbers for targeted editing
This tool returns line numbers where patterns match, enabling precise line-based edits.

SEARCH MODES:
- regex (default): Use regular expressions for pattern matching
- exact: Literal string matching (case-sensitive unless ignoreCase=true)

PARAMETERS:
- path: File to search (relative to workspace root)
- pattern: Search pattern (regex or exact string)
- mode: 'regex' or 'exact' (default: 'regex')
- ignoreCase: Case-insensitive search (default: false)
- lineNumbers: Include line numbers (default: true)

RETURNS:
- Line numbers of matches
- Matched content with ±2 lines of context
- Formatted code blocks for readability

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

NOTE: This tool searches FILE CONTENT and returns LINE NUMBERS with context. For finding files by filename pattern, use searchFiles (which returns file paths).".to_string(),
        input_schema: object_schema(props, vec!["path".to_string(), "pattern".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

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

    // Define the edits array schema
    let mut edit_item_props = HashMap::new();
    edit_item_props.insert(
        "line".to_string(),
        integer_prop(Some(1), None, Some("Line number to edit (1-based)")),
    );
    edit_item_props.insert(
        "old_value".to_string(),
        string_prop(
            None,
            None,
            Some("Optional: Expected current line content for validation. If provided, must match exactly."),
        ),
    );
    edit_item_props.insert(
        "new_value".to_string(),
        string_prop(
            None,
            None,
            Some("New line content (single-line only, no newline characters)"),
        ),
    );

    let edit_item_schema = object_schema(
        edit_item_props,
        vec!["line".to_string(), "new_value".to_string()],
    );

    props.insert(
        "edits".to_string(),
        array_schema(
            edit_item_schema,
            Some(
                "Array of line edit operations. Each edit must have 'line' and 'new_value' fields.",
            ),
        ),
    );

    MCPTool {
        name: "editLineInFile".to_string(),
        title: Some("Edit Multiple Lines in File".to_string()),
        description: "Edit multiple lines in a file atomically using line numbers. All edits succeed or all fail.

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
  - old_value: Optional validation - must match current content exactly
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
    {\"line\": 10, \"old_value\": \"old text\", \"new_value\": \"new text\"},
    {\"line\": 25, \"new_value\": \"another line\"}
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
