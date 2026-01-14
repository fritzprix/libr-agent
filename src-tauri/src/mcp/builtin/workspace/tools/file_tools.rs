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

PARAMETERS:
- path: Relative path from workspace root
- startLine (optional): Read from this line number (1-based)
- endLine (optional): Read up to this line number (1-based)

USAGE:
- Use readFile(path) to read entire file
- Use readFile(path, startLine, endLine) to read specific line ranges
- Line ranges are inclusive [startLine, endLine]

⚠️ PREREQUISITE: File must exist in workspace
💡 NEXT: Use writeFile to modify content or replaceStringInFile for targeted edits"
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
            Some("Relative path to the file to write (from workspace root)"),
        ),
    );
    props.insert(
        "content".to_string(),
        string_prop(
            None,
            None,
            Some("Content to write to the file. Actual maximum is enforced server-side via LIBRAGENT_MAX_FILE_SIZE"),
        ),
    );
    props.insert(
        "mode".to_string(),
        string_prop(
            None,
            None,
            Some("Write mode: 'w' for overwrite (default), 'a' for append"),
        ),
    );

    MCPTool {
        name: "writeFile".to_string(),
        title: Some("Write File".to_string()),
        description: "Write content to a file in the workspace. Creates file if it doesn't exist.

MODES:
- 'w' (default): Overwrites entire file with new content
- 'a': Appends content to end of existing file

⚠️ CRITICAL WORKFLOW FOR EDITS:
1. Call readFile(path) FIRST to see current content
2. Modify content as needed
3. Call writeFile(path, newContent, 'w') to save

⚠️ WARNING: Mode 'w' replaces ALL file content - use replaceStringInFile for targeted edits
💡 NEXT: Use readFile to verify changes or listDirectory to see workspace structure"
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

USAGE:
- Use listDirectory('.') to see workspace root contents
- Use listDirectory('src') to explore subdirectories
- Navigate deeper by concatenating paths: 'src/components'

💡 NEXT: Use readFile to examine file contents or listDirectory on subdirectories to explore deeper".to_string(),
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

pub fn create_replace_string_in_file_tool() -> MCPTool {
    let mut item_props = HashMap::new();
    item_props.insert(
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

❌ NEVER use text reconstructed from previous attempts
✅ ALWAYS use text exactly as shown in readFile response"),
        ),
    );
    item_props.insert(
        "newString".to_string(),
        string_prop(
            None,
            None,
            Some("New text content to replace oldString with. Use empty string to delete the matched text."),
        ),
    );

    let replacement_item_schema = object_schema(
        item_props,
        vec!["oldString".to_string(), "newString".to_string()],
    );

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
        "replacements".to_string(),
        array_schema(
            replacement_item_schema,
            Some("An array of string replacement objects"),
        ),
    );

    MCPTool {
        name: "replaceStringInFile".to_string(),
        title: Some("Replace String in File".to_string()),
        description: "Replace text content in a file using exact string matching. More robust than line-based replacement as it works regardless of line number changes. Supports multiple independent replacements in a single call.

⚠️ CRITICAL WORKFLOW (MUST FOLLOW):
1. ALWAYS call readFile(path) or readFile(path, startLine, endLine) FIRST
2. Extract the exact text from readFile response into oldString parameter
3. Verify the extracted text includes surrounding context (3-5 lines) for uniqueness
4. Then call replaceStringInFile with the extracted oldString

❌ NEVER use oldString reconstructed from previous attempts or assumptions
✅ ALWAYS use text exactly as returned by readFile to ensure exact match

⚠️ ERROR RECOVERY:
- If 'Pattern not found' error: Call readFile again to get updated content
- If 'Multiple matches' error: Include more surrounding context (5-10 lines)
- If 'File changed' error: Re-read file before retrying
- DO NOT retry with same oldString after failure

💡 NEXT: Use previewReplacement to verify changes before committing or readFile to confirm edits".to_string(),
        input_schema: object_schema(props, vec!["path".to_string(), "replacements".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_preview_replacement_tool() -> MCPTool {
    let mut item_props = HashMap::new();
    item_props.insert(
        "oldString".to_string(),
        string_prop(
            None,
            None,
            Some("Text content you want to find and replace. Extract from readFile response."),
        ),
    );
    item_props.insert(
        "newString".to_string(),
        string_prop(
            None,
            None,
            Some("New text content to replace oldString with."),
        ),
    );

    let replacement_item_schema = object_schema(
        item_props,
        vec!["oldString".to_string(), "newString".to_string()],
    );

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
        "replacements".to_string(),
        array_schema(
            replacement_item_schema,
            Some("Array of replacement objects to preview"),
        ),
    );

    MCPTool {
        name: "previewReplacement".to_string(),
        title: Some("Preview File Replacement".to_string()),
        description: "Preview what would change if replaceStringInFile is executed. Shows exact diffs without modifying the file.

🎯 USE CASE: Verify oldString matches before committing changes

WORKFLOW:
1. Call readFile(path) to get current content
2. Call previewReplacement(path, oldString, newString) to see what would change
3. Review the diff output (shows ± lines with context)
4. If preview looks correct, call replaceStringInFile with SAME parameters

✅ BENEFITS:
- Catch mismatches early without file corruption
- See exact line numbers and context
- Verify oldString was extracted correctly from readFile

⚠️ READ-ONLY: This tool does NOT modify files, only shows preview".to_string(),
        input_schema: object_schema(props, vec!["path".to_string(), "replacements".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
