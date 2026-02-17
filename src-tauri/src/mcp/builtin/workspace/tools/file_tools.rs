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
    props.insert(
        "showLineNumbers".to_string(),
        boolean_prop(Some(
            "If true, includes line numbers in the output (default: false)",
        )),
    );
    props.insert(
        "showHash".to_string(),
        boolean_prop(Some(
            "If true, includes short MD5 hash for each line. REQUIRED for editing with editLineInFile.",
        )),
    );

    MCPTool {
        name: "readFile".to_string(),
        title: Some("Read File".to_string()),
        description: "Read the contents of a file from the workspace. Returns file content as text. Supports optional line range reading.".to_string(),
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
            Some("Relative path from workspace root. Examples: 'src/main.rs', 'config.json'"),
        ),
    );
    props.insert(
        "content".to_string(),
        string_prop(
            None,
            None,
            Some("Content to write to the file. Maximum size enforced server-side."),
        ),
    );
    props.insert(
        "overwrite".to_string(),
        boolean_prop(Some("Allow overwriting existing files? (default: false)")),
    );

    MCPTool {
        name: "writeFile".to_string(),
        title: Some("Write File".to_string()),
        description: "Create a new file or overwrite an existing one (if overwrite=true). Returns success status and diffs.".to_string(),
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
        description: "List all files and subdirectories in a workspace directory. Returns names and types (file/directory).".to_string(),
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
            Some("Exact text content to find and replace. Must match precisely including whitespace."),
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
    props.insert(
        "dryRun".to_string(),
        boolean_prop(Some(
            "If true, returns a preview of the changes without modifying the file (default: false)",
        )),
    );

    MCPTool {
        name: "editFile".to_string(),
        title: Some("Edit File (Deprecated)".to_string()),
        description: "[DEPRECATED] Replace a single string in a file. Use `editLineInFile` instead for safer, hash-verified editing.".to_string(),
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
        description:
            "Search for text patterns in a file and get matching line numbers with context."
                .to_string(),
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
        "expected_hash".to_string(),
        string_prop(
            None,
            None,
            Some("Optional: Expected MD5 hash (first 4 chars) of the line. If provided, must match current line hash."),
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
        vec![
            "line".to_string(),
            "new_value".to_string(),
            "expected_hash".to_string(),
        ],
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
        description: "Edit multiple lines in a file atomically using line numbers. All edits succeed or all fail.".to_string(),
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
        description: "Delete a file from the workspace. Permanently removes the file.".to_string(),
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
        title: Some("Edit File (Multiple Replacements) [Deprecated]".to_string()),
        description: "[DEPRECATED] Apply multiple text replacements. Use `editLineInFile` instead for safer, atomic, hash-verified editing.".to_string(),
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
        description:
            "Find files and directories using glob patterns. Searches for FILE NAMES, not content."
                .to_string(),
        input_schema: object_schema(props, vec!["pattern".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
