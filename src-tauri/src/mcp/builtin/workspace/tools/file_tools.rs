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

USAGE:
- Use readFile(path) to read entire file
- Use readFile(path, startLine, endLine) to read specific line ranges
- Line ranges are inclusive [startLine, endLine]

❌ NEVER edit files without reading them first
✅ ALWAYS use readFile before any edit operation

💡 NEXT: Use editFile for targeted changes or writeFile for new files"
            .to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_write_file_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some(
                "Relative path from workspace root. Examples: 'src/main.rs', 'config.json'

⚠️ VALIDATION:
- Must be relative path (no '../' traversal)
- Validated against strict security rules
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
            Some(
                "Content to write to the file. Maximum size enforced server-side.

⚠️ LIMITS:
- String input only
- For diff generation on overwrite, old content is read first

💡 TIP: Empty content creates an empty file",
            ),
        ),
    );
    props.insert(
        "overwrite".to_string(),
        boolean_prop(Some(
            "Allow overwriting existing files? (default: false)
- false: Fails if file exists (Safety Check)
- true: Overwrites and returns exact diff of changes

⚠️ DESTRUCTIVE: Use with caution when true",
        )),
    );

    MCPTool {
        name: "writeFile".to_string(),
        title: Some("Write File".to_string()),
        description: "Create a new file or overwrite an existing one. Returns success status and differences if overwritten.

⚠️ CRITICAL BEHAVIOR:
- Default (overwrite=false): FAILS if file exists (Safe Mode)
- Overwrite (overwrite=true): Replaces ENTIRE content and returns Diff
- Atomic operation: All or nothing

💡 WORKFLOW:
1. New File: writeFile(path, content)
2. Overwrite: writeFile(path, content, overwrite=true)
3. Incremental Edit: Use editFile instead (safer)

✅ RESPONSE:
- Returns verified path, size, and line count
- If overwritten: Returns GIT-STYLE DIFF of changes
- If truncated: Returns preview + size info

💡 NEXT: Use readFile to verify or editFile for refinements"
            .to_string(),
        input_schema: object_schema(
            props,
            vec!["path".to_string(), "content".to_string()],
        ),
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

pub fn create_delete_file_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path to the file to delete (from workspace root)"),
        ),
    );

    MCPTool {
        name: "deleteFile".to_string(),
        title: Some("Delete File".to_string()),
        description: "Delete a file from the workspace. Permanently removes the file.

⚠️ CRITICAL WARNING:
- This operation is DESTRUCTIVE and PERMANENT
- Deleted files cannot be recovered through this tool
- Use with extreme caution

💡 WORKFLOW:
1. ALWAYS verify file exists: listDirectory or readFile first
2. Consider backing up important files before deletion
3. For complete file replacement: deleteFile → writeFile

USAGE SCENARIOS:
✅ Removing temporary/test files
✅ Cleaning up old/unused files
✅ Complete file replacement workflow
✅ Removing generated/build artifacts

❌ AVOID:
- Deleting files without verification
- Removing system or critical project files
- Using as undo mechanism (not recoverable)

🔗 RELATED TOOLS:
- listDirectory: Verify file exists before deletion
- readFile: Check file content before deletion
- createFile: Create new file after deletion (replacement workflow)

💡 TIP: For partial content changes, use editFile instead of deleteFile + createFile"
            .to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_edit_file_multi_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path to the file to modify (from workspace root)"),
        ),
    );
    // Define the replacement item object schema
    let mut replacement_props = HashMap::new();
    replacement_props.insert(
        "oldString".to_string(),
        string_prop(
            None,
            None,
            Some("Exact text to find and replace (must match precisely including whitespace)"),
        ),
    );
    replacement_props.insert(
        "newString".to_string(),
        string_prop(None, None, Some("New text to replace oldString with")),
    );

    let replacement_item = object_schema(
        replacement_props,
        vec!["oldString".to_string(), "newString".to_string()],
    );

    props.insert(
        "replacements".to_string(),
        array_schema(
            replacement_item,
            Some("Array of replacements to apply sequentially. Each replacement must specify oldString and newString (max 50)."),
        ),
    );

    MCPTool {
        name: "editFileMulti".to_string(),
        title: Some("Edit File (Multiple Replacements)".to_string()),
        description: "Apply multiple text replacements to a file in a single atomic operation. All replacements succeed or none are applied.

⚠️ CRITICAL WORKFLOW:
1. ALWAYS call readFile(path) FIRST to get current content
2. Extract exact text for EACH oldString from readFile response
3. Each oldString must match exactly once in the file
4. Replacements are applied sequentially in array order

BEHAVIOR:
✅ All patterns valid → All replacements applied atomically
❌ Any pattern invalid → NO changes applied, detailed error returned

VALIDATION RULES:
- Each oldString must exist exactly once (not 0, not 2+)
- Empty oldString not allowed
- Order matters: Later replacements see results of earlier ones
- Maximum 50 replacements per call

💡 WHEN TO USE:
✅ Batch editing multiple sections in same file
✅ Refactoring with multiple related changes
✅ Applying systematic updates (e.g., renaming across file)
✅ When atomic all-or-nothing behavior is required

❌ WHEN NOT TO USE:
- Single replacement → use editFile (simpler)
- Changes in different files → call editFile separately
- Line-based edits with known line numbers → use editLineInFile

EXAMPLE:
{
  \"path\": \"src/main.rs\",
  \"replacements\": [
    {\"oldString\": \"fn old_name()\", \"newString\": \"fn new_name()\"},
    {\"oldString\": \"old_name()\", \"newString\": \"new_name()\"}
  ]
}

🔗 RELATED TOOLS:
- editFile: Single replacement (simpler, recommended for most cases)
- previewReplacement: Preview single replacement before applying
- readFile: Get current content (MANDATORY before editing)

💡 TIP: For 1-2 changes, use editFile multiple times. For 3+ related changes, use editFileMulti.".to_string(),
        input_schema: object_schema(props, vec!["path".to_string(), "replacements".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_search_files_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path to the directory to search in (from workspace root)"),
        ),
    );
    props.insert(
        "pattern".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Glob pattern to match file names (e.g., '*.rs', 'src/**/*.ts')"),
        ),
    );
    props.insert(
        "max_depth".to_string(),
        integer_prop(
            Some(1),
            Some(100),
            Some("Maximum depth to traverse (optional)"),
        ),
    );
    props.insert(
        "file_type".to_string(),
        string_prop(
            None,
            None,
            Some("Type of files to search: 'file', 'dir', or 'both' (default: 'both')"),
        ),
    );

    MCPTool {
        name: "searchFiles".to_string(),
        title: Some("Search Files by Name".to_string()),
        description: "Find files and directories using glob patterns.
        
⚠️ PRIMARY USE CASE: Finding files when you don't know the exact path
This tool searches for FILE NAMES, not content.

RETURNS:
- List of matching file paths
- File types (file/directory)
- File sizes

💡 WORKFLOW:
1. Use searchFiles to find file paths
2. Use readFile to view content of found files
3. Use searchLineInFile to search WITHIN files

EXAMPLES:
- searchFiles({pattern: '*.rs'}) → Find all Rust files in root
- searchFiles({pattern: 'src/**/*.ts', path: '.'}) → Find all TS files in src recursively

✅ BEST FOR: Locating files by name or extension
❌ NOT FOR: Searching text content inside files (use searchLineInFile)

💡 TIP: Use '**' in pattern for recursive search (e.g., '**/*.json')"
            .to_string(),
        input_schema: object_schema(props, vec!["pattern".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
