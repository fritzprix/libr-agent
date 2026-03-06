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
            "Show decorated line numbers in output (e.g. '  42 | code'). Use showLineHashes instead for replaceLines workflows.",
        )),
    );
    props.insert(
        "showLineHashes".to_string(),
        boolean_prop(Some(
            "Optional: include a 2-char hash for each line (e.g. '42:a3|...'). Use this when you plan to edit specific lines with high precision using replaceLines.",
        )),
    );

    MCPTool {
        name: "readFile".to_string(),
        title: Some("Read File".to_string()),
        description: "Read the contents of a file.

- For general reading: just provide the 'path'.
- For precise editing: set 'showLineHashes: true' to get staleness-safe identifiers for replaceLines.
- For large files: use 'startLine' and 'endLine' to read specific segments."
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
            Some("Relative path from workspace root (e.g. 'src/main.rs')"),
        ),
    );
    props.insert(
        "content".to_string(),
        string_prop(
            None,
            None,
            Some("File content to write. Empty string creates an empty file."),
        ),
    );
    props.insert(
        "overwrite".to_string(),
        boolean_prop(Some(
            "Allow overwriting existing files (default: false). When true, replaces entire content and returns a diff.",
        )),
    );

    MCPTool {
        name: "writeFile".to_string(),
        title: Some("Write File".to_string()),
        description: "Create a new file or overwrite an existing one.

- overwrite=false (default): fails if file already exists
- overwrite=true: replaces entire content, returns a diff

Use replaceLines for targeted edits instead of full overwrites."
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
        description: "List files and subdirectories in a workspace directory.

- listDirectory('.') — workspace root
- listDirectory('src/components') — subdirectory

Returns names and types (file/directory). Use searchFiles for glob-based filtering."
            .to_string(),
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

pub fn create_search_lines_tool() -> MCPTool {
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
        name: "searchLines".to_string(),
        title: Some("Search Lines in File".to_string()),
        description: "Search for text patterns within a file. Returns matching line numbers and surrounding context.

Modes: `regex` (default) or `exact`. Set `ignoreCase=true` for case-insensitive.

Use the returned line numbers directly in replaceLines. For finding files by name, use searchFiles instead.".to_string(),
        input_schema: object_schema(props, vec!["path".to_string(), "pattern".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_replace_lines_tool() -> MCPTool {
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
        "insertAfter".to_string(),
        boolean_prop(Some(
            "Insert-after mode (default: false). When true, new_value is inserted AFTER line N without modifying it. line_hash validates the anchor line for staleness. new_value may contain \\n.",
        )),
    );
    edit_item_props.insert(
        "line".to_string(),
        integer_prop(Some(1), None, Some("Start line number (1-based, required). Must reference an existing line. For range edit this is the first line of the replaced range. To append at end, use insertAfter=true with the current last line as anchor.")),
    );
    edit_item_props.insert(
        "endLine".to_string(),
        integer_prop(Some(1), None, Some("End line number (1-based, optional). When provided, lines [line..endLine] are replaced with new_value. new_value may contain \\n for multi-line replacement. Must be ≥ line.")),
    );
    edit_item_props.insert(
        "line_hash".to_string(),
        string_prop(
            None,
            None,
            Some("Optional: 2-char FNV-1a hash of the START line from readFile(showLineHashes=true). Detects staleness. Alias: 'startHash'. Copy directly from '{N}:{hash}|content' prefix."),
        ),
    );
    edit_item_props.insert(
        "endHash".to_string(),
        string_prop(
            None,
            None,
            Some("Optional: 2-char hash of the END line (range mode only). Provides staleness detection on both boundaries."),
        ),
    );
    edit_item_props.insert(
        "old_value".to_string(),
        string_prop(
            None,
            None,
            Some("Optional: exact current content of the line for validation (single-line mode only). Ignored in range mode. Prefer line_hash instead."),
        ),
    );
    edit_item_props.insert(
        "new_value".to_string(),
        string_prop(
            None,
            None,
            Some("Replacement content. Use empty string \"\" to DELETE the line(s). Single-line mode: no \\n allowed. Range mode (endLine present): \\n is allowed and each \\n-separated segment becomes a new line."),
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
        name: "replaceLines".to_string(),
        title: Some("Edit Multiple Lines in File".to_string()),
        description: r#"Edit specific lines in a file. Edits are atomic (all-or-nothing).

MODES:
  Replace: { line, new_value }
  Insert:  { line, insertAfter: true, new_value }
  Delete:  { line, new_value: "" }
  Range:   { line, endLine, new_value } (replaces multiple lines)

SAFETY (Optional):
  Provide 'line_hash' from readFile(showLineHashes=true) to ensure you are editing the exact version of the line you saw.

Example:
  { path: 'main.rs', edits: [{ line: 10, new_value: 'let x = 1;' }] }"#.to_string(),
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
        description: "Permanently delete a file from the workspace. Irreversible.

For partial content changes, use replaceLines instead."
            .to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
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
        description: "Find files and directories by glob pattern. Returns paths, not content.

- searchFiles({pattern: '*.rs'}) — all Rust files in root
- searchFiles({pattern: 'src/**/*.ts'}) — recursive TS files
- Use `**` for recursive search

For searching text inside files, use searchLines."
            .to_string(),
        input_schema: object_schema(props, vec!["pattern".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
