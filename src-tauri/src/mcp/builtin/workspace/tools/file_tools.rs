use crate::mcp::{schema::JSONSchema, utils::schema_builder::*, MCPTool};

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
            Some("Path to the file to read. Relative paths resolve from the workspace; absolute paths are also allowed unless protected."),
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
            "Optional: include opaque edit anchors for each line in the form '42:a31f2c|...'. Here '42' is the line number and 'a31f2c' is the anchor. For edit tools, pass only the 6-character anchor (for example 'a31f2c'), not '42:a31f2c' or the trailing '|...'.",
        )),
    );

    MCPTool {
        name: "readFile".to_string(),
        title: Some("Read File".to_string()),
        description: "Read the contents of a file. Large responses are chunked automatically to stay inline; use the returned startLine/endLine guidance to continue reading. Use showLineAnchors=true when you need anchors for editFiles."
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
            Some("Path to write. Relative paths resolve from the workspace; absolute paths are also allowed unless protected."),
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
            Some("Write mode. 'create' fails if the file already exists, 'overwrite' replaces the entire file, and 'append' adds content to the end."),
        ),
    );

    MCPTool {
        name: "writeFile".to_string(),
        title: Some("Write File".to_string()),
        description: "Create, overwrite, or append content to a file. Missing parent directories are created automatically. Responses include current line anchors so follow-up editFiles calls can usually reuse them directly. mode='overwrite' returns a diff of the changes."
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
            Some("Path to the directory to list. Relative paths resolve from the workspace; absolute paths are also allowed unless protected."),
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

- listDirectory('.') — workspace directory
- listDirectory('src/components') — subdirectory
- listDirectory('/tmp') — absolute directory

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
            Some("Path to the file or directory to search. Relative paths resolve from the workspace; absolute paths are also allowed unless protected."),
        ),
    );
    props.insert(
        "limit".to_string(),
        integer_prop(
            Some(1),
            Some(1000),
            Some("Maximum number of file entries to return (default: 50). For content search, this limits files with matches, not individual matching lines."),
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
            Some("Regular expression pattern to search for text inside files. Matched against full file content with multiline mode enabled, so ^ and $ match line boundaries. '.' does not match newlines unless you opt into that in the regex itself (for example with (?s)). If omitted, only searches for file names."),
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
            "Include edit anchors in results for use with editFiles (default: false). Anchored lines look like '42:a31f2c|...'; for edit tools, pass only the 6-character anchor (for example 'a31f2c').",
        )),
    );

    MCPTool {
        name: "search".to_string(),
        title: Some("Search Workspace".to_string()),
        description: "Search files by name or content. Content search uses regex against full file text with multiline mode enabled, while results are still reported as line-based hits. Relative paths resolve from the workspace; absolute paths are also allowed unless protected.".to_string(),
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
            Some("Path to the file to edit. Relative paths resolve from the workspace; absolute paths are also allowed unless protected."),
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
            Some("Path to the file to edit. Relative paths resolve from the workspace; absolute paths are also allowed unless protected."),
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
            Some("Path to the file to edit. Relative paths resolve from the workspace; absolute paths are also allowed unless protected."),
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

fn create_edit_item_schema(path_required: bool) -> JSONSchema {
    let path_desc =
        "Path to the file to edit. Relative paths resolve from the workspace; absolute paths are also allowed unless protected.";
    let start_line_desc = "Target start line number. Existing lines are 1-based. Use 0 only to prepend at the file top; to insert below an existing line, keep that line's 1-based number and set op='insert_after'.";
    let end_line_desc =
        "Inclusive end line for a multi-line replace/delete range. Omit for a single-line edit.";
    let start_anchor_desc =
        "6-character opaque anchor for the start line from readFile(showLineAnchors=true). Required for edits that touch an existing line. Do not include the line number or '|content'.";
    let end_anchor_desc =
        "6-character opaque anchor for the end line. Required when endLine is set for a multi-line replace/delete range.";
    let content_desc =
        "Replacement or inserted content. Omit it to delete. Existing lines stay 1-based; use startLine=0 only for prepend, or keep a 1-based existing line number with op='insert_after' to insert below it.";

    let mut props = HashMap::new();
    if path_required {
        props.insert(
            "path".to_string(),
            string_prop(Some(1), Some(1000), Some(path_desc)),
        );
    }
    props.insert(
        "op".to_string(),
        enum_prop_optional(
            vec!["replace", "insert_after", "delete"],
            Some(
                "Optional operation hint. The server infers replace when content is present, delete when content is omitted, and top-of-file insert when startLine is 0. Use op='insert_after' for inserting below an existing line.",
            ),
        ),
    );
    props.insert(
        "startLine".to_string(),
        integer_prop(Some(0), None, Some(start_line_desc)),
    );
    props.insert(
        "endLine".to_string(),
        integer_prop(Some(1), None, Some(end_line_desc)),
    );
    props.insert(
        "startAnchor".to_string(),
        string_prop(None, None, Some(start_anchor_desc)),
    );
    props.insert(
        "endAnchor".to_string(),
        string_prop(None, None, Some(end_anchor_desc)),
    );
    props.insert(
        "content".to_string(),
        string_prop(None, None, Some(content_desc)),
    );

    let mut schema = object_schema(
        props,
        if path_required {
            vec!["path".to_string(), "startLine".to_string()]
        } else {
            vec!["startLine".to_string()]
        },
    );
    schema.description = Some(
        "A single edit operation. Provide path + startLine, plus anchors for existing-line edits. Existing content uses 1-based line numbers; only startLine=0 prepends at the file top. op is optional for common replace/delete flows but still useful for insert_after.".to_string(),
    );
    schema
}

pub fn create_edit_file_input_schema() -> JSONSchema {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Path to the file to edit. Relative paths resolve from the workspace; absolute paths are also allowed unless protected."),
        ),
    );
    props.insert(
        "edits".to_string(),
        array_schema(
            create_edit_item_schema(false),
            Some("Ordered list of edit operations to apply atomically to one file. All edits are schema-validated and anchor-validated before any write. Edits must not overlap."),
        ),
    );

    object_schema(props, vec!["path".to_string(), "edits".to_string()])
}

pub fn create_edit_files_input_schema() -> JSONSchema {
    let edit_item_schema = create_edit_item_schema(true);

    let mut props = HashMap::new();
    props.insert(
        "edits".to_string(),
        array_schema(
            edit_item_schema,
            Some("Ordered list of edit operations to apply atomically across one or more files. Every item includes its own path. All edits are schema-validated and anchor-validated before any write. Edits must not overlap within the same file."),
        ),
    );

    object_schema(props, vec!["edits".to_string()])
}

pub fn create_edit_files_tool() -> MCPTool {
    MCPTool {
        name: "editFiles".to_string(),
        title: Some("Edit Files (Batch)".to_string()),
        description: "Apply multiple line edits across one or more files atomically in a single operation.

PREREQUISITE: Obtain anchors from a prior readFile(showLineAnchors=true), writeFile response, or previous editFiles response. Anchored lines look like `42:a31f2c|...`; for anchors, pass only the 6-character code such as `a31f2c`, not `42:a31f2c`.

Line numbering is 1-based for existing content. The only valid 0 value is startLine=0, which prepends at the file top.

Each edit item carries its own path. Keep the payload simple and let the server infer the common cases:
- replace single line: path + startLine + startAnchor + content
- replace range: path + startLine + startAnchor + endLine + endAnchor + content
- delete single line: path + startLine + startAnchor
- delete range: path + startLine + startAnchor + endLine + endAnchor
- prepend at file top: path + startLine=0 + content
- insert below an existing line: add op='insert_after' + path + startLine + startAnchor + content

All files are validated before any write begins."
            .to_string(),
        input_schema: create_edit_files_input_schema(),
        output_schema: None,
        annotations: None,
    }
}
