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
        "showLineHashes".to_string(),
        boolean_prop(Some(
            "Optional: include a 2-char hash for each line (e.g. '42:a3|...'). Use this when you plan to edit specific lines with high precision using editFile.",
        )),
    );

    MCPTool {
        name: "readFile".to_string(),
        title: Some("Read File".to_string()),
        description: "Read the contents of a file.

- For general reading: just provide the 'path'.
- For precise editing: set 'showLineHashes: true' to get staleness-safe identifiers for editFile.
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
        "mode".to_string(),
        enum_prop(
            vec!["create", "overwrite", "append"],
            "create",
            Some("Write mode: 'create' (fails if exists), 'overwrite' (replaces entire content), or 'append' (adds to end)."),
        ),
    );

    MCPTool {
        name: "writeFile".to_string(),
        title: Some("Write File".to_string()),
        description: "Create, overwrite, or append content to a file.

- mode='create' (default): fails if file already exists
- mode='overwrite': replaces entire content, returns a diff
- mode='append': adds content to the end of the file"
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

pub fn create_import_files_tool() -> MCPTool {
    let mut file_item_props = HashMap::new();
    file_item_props.insert(
        "srcAbsPath".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Absolute path of source file to import"),
        ),
    );
    file_item_props.insert(
        "destRelPath".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path in workspace where file will be imported"),
        ),
    );

    let mut props = HashMap::new();
    props.insert(
        "files".to_string(),
        array_schema(
            object_schema(
                file_item_props,
                vec!["srcAbsPath".to_string(), "destRelPath".to_string()],
            ),
            Some("List of files to import into the workspace"),
        ),
    );

    MCPTool {
        name: "importFiles".to_string(),
        title: Some("Import Files".to_string()),
        description:
            "Import multiple external files into the workspace in a single batch operation."
                .to_string(),
        input_schema: object_schema(props, vec!["files".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_search_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path to the file or directory to search (from workspace root)"),
        ),
    );
    props.insert(
        "query".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Regular expression pattern to search for text inside files. If omitted, only searches for file names."),
        ),
    );
    props.insert(
        "filePattern".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Glob pattern to filter files by name (e.g. '*.rs', 'src/**/*.ts')."),
        ),
    );
    props.insert(
        "ignoreCase".to_string(),
        boolean_prop(Some("Case-insensitive search (default: false)")),
    );
    props.insert(
        "showLineHashes".to_string(),
        boolean_prop(Some(
            "Include line hashes in results for use with editFile (default: false)",
        )),
    );

    MCPTool {
        name: "search".to_string(),
        title: Some("Search Workspace".to_string()),
        description: "Search for files by name, or search inside files for text patterns.

- To find files: search({path: '.', filePattern: '*.rs'})
- To search text inside a specific file: search({path: 'src/main.rs', query: 'fn main'})
- To search text inside files matching a pattern: search({path: '.', query: 'TODO', filePattern: '*.ts'})

Use the returned line hashes directly in editFile.".to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
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
            Some("Relative path to the file to edit (from workspace root)"),
        ),
    );

    // Define the edits array schema
    let mut edit_item_props = HashMap::new();
    edit_item_props.insert(
        "action".to_string(),
        string_prop(
            None,
            None,
            Some("Type of edit: 'REPLACE' (default), 'INSERT_AFTER', or 'DELETE'. 'REPLACE' swaps lines [line..endLine] with 'new_value'. 'INSERT_AFTER' adds 'new_value' after 'line'. 'DELETE' removes lines [line..endLine]."),
        ),
    );
    edit_item_props.insert(
        "line".to_string(),
        integer_prop(Some(0), None, Some("Start line number (1-based, required). Use line: 0 ONLY with action='INSERT_AFTER' to insert at the very beginning of the file. For range edits, this is the first line of the affected range.")),
    );
    edit_item_props.insert(
        "endLine".to_string(),
        integer_prop(Some(1), None, Some("End line number (1-based, optional). For REPLACE and DELETE ranges (inclusive). Defaults to 'line'. Ignored for INSERT_AFTER.")),
    );
    edit_item_props.insert(
        "new_value".to_string(),
        string_prop(
            None,
            None,
            Some("New content for the line(s). Required for REPLACE and INSERT_AFTER. For single-line REPLACE, must not contain \\n. Use \"\" for DELETE (though 'action' is preferred)."),
        ),
    );
    edit_item_props.insert(
        "line_hash".to_string(),
        string_prop(
            None,
            None,
            Some("Staleness-safe 2-char hash from readFile(showLineHashes=true). Strongly recommended to prevent editing the wrong version of the file. Alias: 'startHash'."),
        ),
    );
    edit_item_props.insert(
        "endHash".to_string(),
        string_prop(
            None,
            None,
            Some("Optional: 2-char hash of the end line (range mode only). Provides staleness detection on both boundaries."),
        ),
    );

    let edit_item_schema = object_schema(
        edit_item_props,
        vec!["line".to_string()], // line is always required
    );

    props.insert(
        "edits".to_string(),
        array_schema(
            edit_item_schema,
            Some(
                "Array of edit operations. Each edit must have 'line' and either 'new_value' or 'action' defined.",
            ),
        ),
    );

    MCPTool {
        name: "editFile".to_string(),
        title: Some("Edit Multiple Lines in File".to_string()),
        description: r#"Advanced line-based editor. Supports atomic replacement, insertion, and deletion.

MODES:
  REPLACE (default): Swaps lines [line..endLine] with new_value.
  INSERT_AFTER:    Adds new_value after 'line'. Use line: 0 to insert at top.
  DELETE:          Removes lines [line..endLine].

SAFETY:
  Provide 'line_hash' from readFile(showLineHashes=true) to detect if the file changed since you last read it.
  The tool handles line-number shifts automatically for multiple edits in one call.

Example (Insert at top):
  { path: 'main.rs', edits: [{ line: 0, action: 'INSERT_AFTER', new_value: '// Copyright 2026\n' }] }"#.to_string(),
        input_schema: object_schema(props, vec!["path".to_string(), "edits".to_string()]),
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

For searching text inside files, use search."
            .to_string(),
        input_schema: object_schema(props, vec!["pattern".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
