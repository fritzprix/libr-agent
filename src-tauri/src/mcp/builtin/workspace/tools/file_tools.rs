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
            "Optional: include opaque edit anchors for each line (e.g. '42:a31f2c|...'). For edit tools, copy only the 6-character anchor between ':' and '|', not the line number or line content.",
        )),
    );

    MCPTool {
        name: "readFile".to_string(),
        title: Some("Read File".to_string()),
        description: "Read the contents of a file. Large responses are chunked automatically to stay inline; use the returned startLine/endLine guidance to continue reading. Use showLineAnchors=true before calling edit tools (replaceLines, insertAfterLine, deleteLines, editFile) to obtain anchor values."
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
        description: "Create, overwrite, or append content to a file. mode='overwrite' returns a diff of the changes."
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
    props.insert(
        "limit".to_string(),
        integer_prop(
            Some(1),
            Some(1000),
            Some("Maximum number of items to return (default: 100)"),
        ),
    );
    props.insert(
        "offset".to_string(),
        integer_prop(
            Some(0),
            None,
            Some("Number of items to skip for pagination (default: 0)"),
        ),
    );

    MCPTool {
        name: "listDirectory".to_string(),
        title: Some("List Directory".to_string()),
        description: "List files and subdirectories in a workspace directory.

- listDirectory('.') — workspace root
- listDirectory('src/components') — subdirectory

Returns names and types (file/directory). Use search with filePattern when you need glob-style filtering."
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
        "limit".to_string(),
        integer_prop(
            Some(1),
            Some(1000),
            Some("Maximum number of results to return (default: 50)"),
        ),
    );
    props.insert(
        "offset".to_string(),
        integer_prop(
            Some(0),
            None,
            Some("Number of results to skip for pagination (default: 0)"),
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
            "Include edit anchors in results for use with replaceLines, insertAfterLine, or deleteLines (default: false). For edit tools, copy only the 6-character anchor between ':' and '|'.",
        )),
    );

    MCPTool {
        name: "search".to_string(),
        title: Some("Search Workspace".to_string()),
        description: "Search workspace files by name or content.".to_string(),
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
            Some("Required. Use only the 6-character opaque anchor from the start line in readFile(showLineAnchors=true) or search(showLineAnchors=true). Do not include the line number or '|content'."),
        ),
    );
    props.insert(
        "endAnchor".to_string(),
        string_prop(
            None,
            None,
            Some("Required when endLine creates a multi-line replacement range. Use only the 6-character opaque anchor from the exact end line in readFile(showLineAnchors=true) or search(showLineAnchors=true). Do not include the line number or '|content'."),
        ),
    );

    // Batch mode: `edits` array where each item has the same fields as the
    // flat single-edit parameters above.
    let mut replace_item_props = HashMap::new();
    replace_item_props.insert(
        "line".to_string(),
        integer_prop(Some(1), None, Some("Start line number (1-based).")),
    );
    replace_item_props.insert(
        "endLine".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("End line for a multi-line range (inclusive). Defaults to 'line'."),
        ),
    );
    replace_item_props.insert(
        "new_value".to_string(),
        string_prop(None, None, Some("Replacement content. May include \\n.")),
    );
    replace_item_props.insert(
        "anchor".to_string(),
        string_prop(
            None,
            None,
            Some("6-character opaque anchor from readFile(showLineAnchors=true) for the start line. Do not include the line number or '|content'."),
        ),
    );
    replace_item_props.insert(
        "endAnchor".to_string(),
        string_prop(
            None,
            None,
            Some("Required when endLine creates a multi-line range. Use only the 6-character anchor from the end line, not the full 'N:anchor|content' string."),
        ),
    );
    props.insert(
        "edits".to_string(),
        array_schema(
            object_schema(
                replace_item_props,
                vec![
                    "line".to_string(),
                    "anchor".to_string(),
                    "new_value".to_string(),
                ],
            ),
            Some("Batch mode: provide multiple replacements to apply atomically. All anchors are validated before any write. Edits must not overlap. Cannot be combined with flat single-edit parameters."),
        ),
    );

    MCPTool {
        name: "replaceLines".to_string(),
        title: Some("Replace Lines".to_string()),
        description: "Replace one line or a contiguous line range with new content.

Use flat params (line/anchor/new_value) for a single edit, or the `edits` array for multiple atomic replacements — cannot combine both modes."
            .to_string(),
        input_schema: object_schema(
            props,
            vec!["path".to_string()],
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
                "Required when afterLine targets an existing line. Use only the 6-character anchor from readFile(showLineAnchors=true) or search(showLineAnchors=true). Omit only when afterLine is 0.",
            ),
        ),
    );

    // Batch mode array items.
    let mut insert_item_props = HashMap::new();
    insert_item_props.insert(
        "afterLine".to_string(),
        integer_prop(
            Some(0),
            None,
            Some("Insert after this line. Use 0 to insert at the very beginning of the file."),
        ),
    );
    insert_item_props.insert(
        "new_value".to_string(),
        string_prop(None, None, Some("Content to insert. May include \\n.")),
    );
    insert_item_props.insert(
        "anchor".to_string(),
        string_prop(
            None,
            None,
            Some(
                "Required when afterLine targets an existing line. Use only the 6-character anchor from readFile(showLineAnchors=true) or search(showLineAnchors=true). Omit only when afterLine is 0.",
            ),
        ),
    );
    props.insert(
        "edits".to_string(),
        array_schema(
            object_schema(
                insert_item_props,
                vec!["afterLine".to_string(), "new_value".to_string()],
            ),
            Some("Batch mode: provide multiple insertions to apply atomically. All anchors are validated before any write. Edits must not overlap. Cannot be combined with flat single-edit parameters."),
        ),
    );

    MCPTool {
        name: "insertAfterLine".to_string(),
        title: Some("Insert After Line".to_string()),
        description: "Insert new content after a specific line without replacing existing content.

Use flat params (afterLine/anchor/new_value) for a single insertion, or the `edits` array for multiple atomic insertions — cannot combine both modes."
            .to_string(),
        input_schema: object_schema(
            props,
            vec!["path".to_string()],
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
            Some("Required. Use only the 6-character opaque anchor from the start line in readFile(showLineAnchors=true) or search(showLineAnchors=true). Do not include the line number or '|content'."),
        ),
    );
    props.insert(
        "endAnchor".to_string(),
        string_prop(
            None,
            None,
            Some("Required when endLine creates a multi-line deletion range. Use only the 6-character opaque anchor from the exact end line in readFile(showLineAnchors=true) or search(showLineAnchors=true). Do not include the line number or '|content'."),
        ),
    );

    // Batch mode array items.
    let mut delete_item_props = HashMap::new();
    delete_item_props.insert(
        "line".to_string(),
        integer_prop(Some(1), None, Some("Start line number (1-based).")),
    );
    delete_item_props.insert(
        "endLine".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("End line for a multi-line range (inclusive). Defaults to 'line'."),
        ),
    );
    delete_item_props.insert(
        "anchor".to_string(),
        string_prop(
            None,
            None,
            Some("6-character opaque anchor from readFile(showLineAnchors=true) for the start line. Do not include the line number or '|content'."),
        ),
    );
    delete_item_props.insert(
        "endAnchor".to_string(),
        string_prop(
            None,
            None,
            Some("Required when endLine creates a multi-line range. Use only the 6-character anchor from the end line, not the full 'N:anchor|content' string."),
        ),
    );
    props.insert(
        "edits".to_string(),
        array_schema(
            object_schema(
                delete_item_props,
                vec!["line".to_string(), "anchor".to_string()],
            ),
            Some("Batch mode: provide multiple deletions to apply atomically. All anchors are validated before any write. Edits must not overlap. Cannot be combined with flat single-edit parameters."),
        ),
    );

    MCPTool {
        name: "deleteLines".to_string(),
        title: Some("Delete Lines".to_string()),
        description: "Delete one line or a contiguous line range.

Use flat params (line/anchor) for a single deletion, or the `edits` array for multiple atomic deletions — cannot combine both modes."
            .to_string(),
        input_schema: object_schema(
            props,
            vec!["path".to_string()],
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_edit_file_tool() -> MCPTool {
    // Compatibility-only batch delegator:
    // editFile remains implemented because older callers and internal batching still route
    // through it, but we intentionally prefer the three dedicated mutation tools for normal
    // model-facing discovery. The reason is contract clarity: action=REPLACE / INSERT_AFTER /
    // DELETE each require different fields and invariants, so splitting them into separate
    // tools keeps each exposed schema narrow and predictable for planning.
    // Build the schema for a single edit item inside the `edits` array.
    let mut edit_item_props = HashMap::new();
    edit_item_props.insert(
        "action".to_string(),
        enum_prop_required(
            vec!["REPLACE", "INSERT_AFTER", "DELETE"],
            "Edit action. REPLACE: swap lines with new_value. INSERT_AFTER: insert below the anchor line. DELETE: remove lines.",
        ),
    );
    edit_item_props.insert(
        "line".to_string(),
        integer_prop(
            Some(0),
            None,
            Some("Target line number (1-based). Use 0 only with INSERT_AFTER to prepend at file top."),
        ),
    );
    edit_item_props.insert(
        "endLine".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("Inclusive end line for a multi-line REPLACE or DELETE range. Omit for single-line operations. Cannot be used with INSERT_AFTER."),
        ),
    );
    edit_item_props.insert(
        "new_value".to_string(),
        string_prop(
            None,
            None,
            Some("Replacement or insertion content. Required for REPLACE and INSERT_AFTER. Omit for DELETE. May contain \\n to span multiple lines."),
        ),
    );
    edit_item_props.insert(
        "anchor".to_string(),
        string_prop(
            None,
            None,
            Some("6-character opaque anchor for the start line from readFile(showLineAnchors=true). Required for all operations except INSERT_AFTER with line=0. Do not include the line number or '|content'."),
        ),
    );
    edit_item_props.insert(
        "endAnchor".to_string(),
        string_prop(
            None,
            None,
            Some("6-character opaque anchor for the end line. Required when endLine is set for a multi-line REPLACE or DELETE. Do not include the line number or '|content'."),
        ),
    );

    let edit_item_schema = object_schema(
        edit_item_props,
        vec!["action".to_string(), "line".to_string()],
    );

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
        array_schema(
            edit_item_schema,
            Some("Ordered list of edit operations to apply atomically. All anchors are validated against the original file before any change is written. Edits must not overlap."),
        ),
    );

    MCPTool {
        name: "editFile".to_string(),
        title: Some("Edit File (Batch)".to_string()),
        description: "Apply multiple line edits to a file atomically in a single operation.

PREREQUISITE: Call readFile(showLineAnchors=true) first to obtain anchor values. For anchors, pass only the 6 hex characters between ':' and '|'.

Edits are applied bottom-to-top so line numbers stay stable within the batch.

Use editFile when making multiple edits to the same file. Use replaceLines / insertAfterLine / deleteLines for a single targeted edit."
            .to_string(),
        input_schema: object_schema(
            props,
            vec!["path".to_string(), "edits".to_string()],
        ),
        output_schema: None,
        annotations: None,
    }
}
