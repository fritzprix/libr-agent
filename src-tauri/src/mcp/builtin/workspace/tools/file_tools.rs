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
        "showLineAnchors".to_string(),
        boolean_prop(Some(
            "Optional: include opaque edit anchors for each line (e.g. '42:a31f2c|...'). Use anchors with replaceLines, insertAfterLine, and deleteLines.",
        )),
    );

    MCPTool {
        name: "readFile".to_string(),
        title: Some("Read File".to_string()),
        description: "Read the contents of a file.

- For general reading: just provide the 'path'.
        - For precise editing: set 'showLineAnchors: true' to get `anchor` values for replaceLines, insertAfterLine, or deleteLines.
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
            Some("Write mode. If omitted, defaults to 'create'. 'create' fails if the file already exists, 'overwrite' replaces the entire file, and 'append' adds content to the end."),
        ),
    );

    MCPTool {
        name: "writeFile".to_string(),
        title: Some("Write File".to_string()),
        description: "Create, overwrite, or append content to a file.

- if 'mode' is omitted, the tool defaults to mode='create'
- mode='create': fails if file already exists
- mode='overwrite': replaces entire content, returns a diff
- mode='append': adds content to the end of the file

Tip: omit 'mode' when you want safe create-only behavior."
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
        "showLineAnchors".to_string(),
        boolean_prop(Some(
            "Include edit anchors in results for use with replaceLines, insertAfterLine, or deleteLines (default: false)",
        )),
    );

    MCPTool {
        name: "search".to_string(),
        title: Some("Search Workspace".to_string()),
        description: "Search for files by name, or search inside files for text patterns.

- To find files: search({path: '.', filePattern: '*.rs'})
- To search text inside a specific file: search({path: 'src/main.rs', query: 'fn main'})
- To search text inside files matching a pattern: search({path: '.', query: 'TODO', filePattern: '*.ts'})

Use the returned anchors directly in replaceLines, insertAfterLine, or deleteLines.".to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
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

    props.insert(
        "line".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("Start line number (1-based, required). For range replacement, this is the first line of the affected range."),
        ),
    );
    props.insert(
        "endLine".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("End line number (1-based, optional). For multi-line replacement ranges (inclusive). Defaults to 'line'."),
        ),
    );
    props.insert(
        "new_value".to_string(),
        string_prop(
            None,
            None,
            Some(
                "Replacement content. Required. May include \\n and can expand or shrink the file.",
            ),
        ),
    );
    props.insert(
        "anchor".to_string(),
        string_prop(
            None,
            None,
            Some("Required. Use the opaque anchor from the start line in readFile(showLineAnchors=true) or search(showLineAnchors=true)."),
        ),
    );
    props.insert(
        "endAnchor".to_string(),
        string_prop(
            None,
            None,
            Some("Required when endLine creates a multi-line replacement range. Use the opaque anchor from the exact end line in readFile(showLineAnchors=true) or search(showLineAnchors=true)."),
        ),
    );

    MCPTool {
        name: "replaceLines".to_string(),
        title: Some("Replace Lines".to_string()),
        description: "Replace one line or a contiguous line range with new content.

- Use this when existing lines should be swapped out.
- For a range replacement, provide line, endLine, anchor, endAnchor, and new_value.
- For a single-line replacement, provide line, anchor, and new_value."
            .to_string(),
        input_schema: object_schema(
            props,
            vec![
                "path".to_string(),
                "line".to_string(),
                "anchor".to_string(),
                "new_value".to_string(),
            ],
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_insert_after_line_tool() -> MCPTool {
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
        "afterLine".to_string(),
        integer_prop(
            Some(0),
            None,
            Some(
                "Insert after this line number. Use 0 to insert at the very beginning of the file.",
            ),
        ),
    );
    props.insert(
        "new_value".to_string(),
        string_prop(
            None,
            None,
            Some("Content to insert. Required and may include \\n."),
        ),
    );
    props.insert(
        "anchor".to_string(),
        string_prop(
            None,
            None,
            Some(
                "Required when afterLine targets an existing line. Omit only when afterLine is 0.",
            ),
        ),
    );

    MCPTool {
        name: "insertAfterLine".to_string(),
        title: Some("Insert After Line".to_string()),
        description: "Insert new content after a specific line without replacing existing content.

- Use afterLine: 0 to insert at the top of the file.
- Use anchor from the exact target line when inserting after an existing line.
- The referenced line stays intact; new content is inserted below it."
            .to_string(),
        input_schema: object_schema(
            props,
            vec![
                "path".to_string(),
                "afterLine".to_string(),
                "new_value".to_string(),
            ],
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_delete_lines_tool() -> MCPTool {
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
        "line".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("Start line number (1-based, required)."),
        ),
    );
    props.insert(
        "endLine".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("End line number (1-based, optional). For multi-line deletion ranges (inclusive). Defaults to 'line'."),
        ),
    );
    props.insert(
        "anchor".to_string(),
        string_prop(
            None,
            None,
            Some("Required. Use the opaque anchor from the start line in readFile(showLineAnchors=true) or search(showLineAnchors=true)."),
        ),
    );
    props.insert(
        "endAnchor".to_string(),
        string_prop(
            None,
            None,
            Some("Required when endLine creates a multi-line deletion range. Use the opaque anchor from the exact end line in readFile(showLineAnchors=true) or search(showLineAnchors=true)."),
        ),
    );

    MCPTool {
        name: "deleteLines".to_string(),
        title: Some("Delete Lines".to_string()),
        description: "Delete one line or a contiguous line range.

- Use line and anchor for single-line deletion.
- Add endLine and endAnchor for multi-line deletion.
- This removes the targeted lines entirely."
            .to_string(),
        input_schema: object_schema(
            props,
            vec!["path".to_string(), "line".to_string(), "anchor".to_string()],
        ),
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
