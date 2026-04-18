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
        description: "Read the contents of a file. Large responses are chunked automatically to stay inline; use the returned startLine/endLine guidance to continue reading. Use showLineAnchors=true before calling editFile to obtain anchor values."
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
            "Include edit anchors in results for use with editFile (default: false). For edit tools, copy only the 6-character anchor between ':' and '|'.",
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

fn build_edit_variant_schema(
    properties: HashMap<String, JSONSchema>,
    required: Vec<String>,
    description: &str,
) -> JSONSchema {
    let mut schema = object_schema(properties, required);
    schema.description = Some(description.to_string());
    schema
}

pub fn create_edit_file_input_schema() -> JSONSchema {
    let start_line_desc =
        "Target start line number (1-based). Use 0 only with op='insert_after' to prepend at the file top.";
    let end_line_desc =
        "Inclusive end line for a multi-line replace/delete range. Omit for a single-line edit.";
    let start_anchor_desc =
        "6-character opaque anchor for the start line from readFile(showLineAnchors=true). Do not include the line number or '|content'.";
    let end_anchor_desc =
        "6-character opaque anchor for the end line. Required when endLine is set for a multi-line replace/delete range.";
    let content_desc = "Replacement or inserted content. May include \\n to span multiple lines.";

    let mut replace_single_props = HashMap::new();
    replace_single_props.insert(
        "op".to_string(),
        string_const_prop(
            "replace",
            Some("Replace one line or a contiguous line range with content."),
        ),
    );
    replace_single_props.insert(
        "startLine".to_string(),
        integer_prop(Some(1), None, Some(start_line_desc)),
    );
    replace_single_props.insert(
        "startAnchor".to_string(),
        string_prop(None, None, Some(start_anchor_desc)),
    );
    replace_single_props.insert(
        "content".to_string(),
        string_prop(None, None, Some(content_desc)),
    );
    let replace_single_schema = build_edit_variant_schema(
        replace_single_props,
        vec![
            "op".to_string(),
            "startLine".to_string(),
            "startAnchor".to_string(),
            "content".to_string(),
        ],
        "Single-line replace variant: requires startLine, startAnchor, and content.",
    );

    let mut replace_range_props = HashMap::new();
    replace_range_props.insert(
        "op".to_string(),
        string_const_prop(
            "replace",
            Some("Replace one line or a contiguous line range with content."),
        ),
    );
    replace_range_props.insert(
        "startLine".to_string(),
        integer_prop(Some(1), None, Some(start_line_desc)),
    );
    replace_range_props.insert(
        "endLine".to_string(),
        integer_prop(Some(1), None, Some(end_line_desc)),
    );
    replace_range_props.insert(
        "startAnchor".to_string(),
        string_prop(None, None, Some(start_anchor_desc)),
    );
    replace_range_props.insert(
        "endAnchor".to_string(),
        string_prop(None, None, Some(end_anchor_desc)),
    );
    replace_range_props.insert(
        "content".to_string(),
        string_prop(None, None, Some(content_desc)),
    );
    let replace_range_schema = build_edit_variant_schema(
        replace_range_props,
        vec![
            "op".to_string(),
            "startLine".to_string(),
            "endLine".to_string(),
            "startAnchor".to_string(),
            "endAnchor".to_string(),
            "content".to_string(),
        ],
        "Multi-line replace variant: requires startLine, endLine, startAnchor, endAnchor, and content.",
    );

    let mut insert_existing_props = HashMap::new();
    insert_existing_props.insert(
        "op".to_string(),
        string_const_prop(
            "insert_after",
            Some("Insert content after an existing line."),
        ),
    );
    insert_existing_props.insert(
        "startLine".to_string(),
        integer_prop(Some(1), None, Some(start_line_desc)),
    );
    insert_existing_props.insert(
        "startAnchor".to_string(),
        string_prop(None, None, Some(start_anchor_desc)),
    );
    insert_existing_props.insert(
        "content".to_string(),
        string_prop(None, None, Some(content_desc)),
    );
    let insert_existing_schema = build_edit_variant_schema(
        insert_existing_props,
        vec![
            "op".to_string(),
            "startLine".to_string(),
            "startAnchor".to_string(),
            "content".to_string(),
        ],
        "Insert-after variant for an existing line: requires startLine, startAnchor, and content.",
    );

    let mut insert_top_props = HashMap::new();
    insert_top_props.insert(
        "op".to_string(),
        string_const_prop(
            "insert_after",
            Some("Insert content at the very beginning of the file."),
        ),
    );
    insert_top_props.insert(
        "startLine".to_string(),
        integer_const_prop(
            0,
            Some("Use startLine=0 with op='insert_after' to prepend at the file top."),
        ),
    );
    insert_top_props.insert(
        "content".to_string(),
        string_prop(None, None, Some(content_desc)),
    );
    let insert_top_schema = build_edit_variant_schema(
        insert_top_props,
        vec![
            "op".to_string(),
            "startLine".to_string(),
            "content".to_string(),
        ],
        "Top-prepend variant: startLine must be 0 and no anchor is required.",
    );

    let mut delete_single_props = HashMap::new();
    delete_single_props.insert(
        "op".to_string(),
        string_const_prop(
            "delete",
            Some("Delete one line or a contiguous line range."),
        ),
    );
    delete_single_props.insert(
        "startLine".to_string(),
        integer_prop(Some(1), None, Some(start_line_desc)),
    );
    delete_single_props.insert(
        "startAnchor".to_string(),
        string_prop(None, None, Some(start_anchor_desc)),
    );
    let delete_single_schema = build_edit_variant_schema(
        delete_single_props,
        vec![
            "op".to_string(),
            "startLine".to_string(),
            "startAnchor".to_string(),
        ],
        "Single-line delete variant: requires startLine and startAnchor.",
    );

    let mut delete_range_props = HashMap::new();
    delete_range_props.insert(
        "op".to_string(),
        string_const_prop(
            "delete",
            Some("Delete one line or a contiguous line range."),
        ),
    );
    delete_range_props.insert(
        "startLine".to_string(),
        integer_prop(Some(1), None, Some(start_line_desc)),
    );
    delete_range_props.insert(
        "endLine".to_string(),
        integer_prop(Some(1), None, Some(end_line_desc)),
    );
    delete_range_props.insert(
        "startAnchor".to_string(),
        string_prop(None, None, Some(start_anchor_desc)),
    );
    delete_range_props.insert(
        "endAnchor".to_string(),
        string_prop(None, None, Some(end_anchor_desc)),
    );
    let delete_range_schema = build_edit_variant_schema(
        delete_range_props,
        vec![
            "op".to_string(),
            "startLine".to_string(),
            "endLine".to_string(),
            "startAnchor".to_string(),
            "endAnchor".to_string(),
        ],
        "Multi-line delete variant: requires startLine, endLine, startAnchor, and endAnchor.",
    );

    let edit_item_schema = one_of_object_schema(
        vec![
            replace_single_schema,
            replace_range_schema,
            insert_existing_schema,
            insert_top_schema,
            delete_single_schema,
            delete_range_schema,
        ],
        Some("A single edit operation. The required fields depend on op."),
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
            Some("Ordered list of edit operations to apply atomically. All edits are schema-validated and anchor-validated before any write. Edits must not overlap."),
        ),
    );

    object_schema(props, vec!["path".to_string(), "edits".to_string()])
}

pub fn create_edit_file_tool() -> MCPTool {
    MCPTool {
        name: "editFile".to_string(),
        title: Some("Edit File (Batch)".to_string()),
        description: "Apply multiple line edits to a file atomically in a single operation.

PREREQUISITE: Call readFile(showLineAnchors=true) first to obtain anchor values. For anchors, pass only the 6 hex characters between ':' and '|'.

Each edit uses op-specific schema validation:
- replace single line: startLine + startAnchor + content
- replace range: startLine + startAnchor + endLine + endAnchor + content
- insert_after existing line: startLine + startAnchor + content
- insert_after file top: startLine=0 + content
- delete single line: startLine + startAnchor
- delete range: startLine + startAnchor + endLine + endAnchor

Edits are applied bottom-to-top so line numbers stay stable within the batch. Legacy replaceLines / insertAfterLine / deleteLines still route here for backward compatibility."
            .to_string(),
        input_schema: create_edit_file_input_schema(),
        output_schema: None,
        annotations: None,
    }
}
