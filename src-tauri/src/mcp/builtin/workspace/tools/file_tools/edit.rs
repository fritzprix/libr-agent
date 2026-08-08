use crate::mcp::{schema::SchemaProperties, utils::schema_builder::*, MCPTool};

#[cfg(feature = "workspace-edit-file")]
use crate::mcp::schema::JSONSchema;

pub fn create_replace_lines_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Path to the file to edit. Relative paths resolve from the workspace; absolute paths are also allowed unless protected. Use @teamwork/... or .libragent/teamwork/... to edit teamwork scaffold files without changing workspaceOverride."),
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
            Some("Required. Use only the 6-character opaque anchor from the start line in workspace__readFile(showLineAnchors=true) or workspace__searchFiles(showLineAnchors=true). Do not include the line number or '|content'."),
        ),
    );
    props.insert(
        "endAnchor".to_string(),
        string_prop(
            None,
            None,
            Some("Required when endLine creates a multi-line replacement range. Use only the 6-character opaque anchor from the exact end line in workspace__readFile(showLineAnchors=true) or workspace__searchFiles(showLineAnchors=true). Do not include the line number or '|content'."),
        ),
    );

    // Batch mode: `edits` array where each item has the same fields as the
    // flat single-edit parameters above.
    let mut replace_item_props = SchemaProperties::new();
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
            Some("6-character opaque anchor from workspace__readFile(showLineAnchors=true) for the start line. Do not include the line number or '|content'."),
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
    let mut props = SchemaProperties::new();
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
                "Required when afterLine targets an existing line. Use only the 6-character anchor from workspace__readFile(showLineAnchors=true) or workspace__searchFiles(showLineAnchors=true). Omit only when afterLine is 0.",
            ),
        ),
    );

    // Batch mode array items.
    let mut insert_item_props = SchemaProperties::new();
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
                "Required when afterLine targets an existing line. Use only the 6-character anchor from workspace__readFile(showLineAnchors=true) or workspace__searchFiles(showLineAnchors=true). Omit only when afterLine is 0.",
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
    let mut props = SchemaProperties::new();
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
            Some("Required. Use only the 6-character opaque anchor from the start line in workspace__readFile(showLineAnchors=true) or workspace__searchFiles(showLineAnchors=true). Do not include the line number or '|content'."),
        ),
    );
    props.insert(
        "endAnchor".to_string(),
        string_prop(
            None,
            None,
            Some("Required when endLine creates a multi-line deletion range. Use only the 6-character opaque anchor from the exact end line in workspace__readFile(showLineAnchors=true) or workspace__searchFiles(showLineAnchors=true). Do not include the line number or '|content'."),
        ),
    );

    // Batch mode array items.
    let mut delete_item_props = SchemaProperties::new();
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
            Some("6-character opaque anchor from workspace__readFile(showLineAnchors=true) for the start line. Do not include the line number or '|content'."),
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

#[cfg(feature = "workspace-str-replace")]
pub fn create_str_replace_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Path to the file to edit. Relative paths resolve from the workspace; absolute paths are also allowed unless protected."),
        ),
    );
    props.insert(
        "replace_all".to_string(),
        boolean_prop(Some(
            "Replace every occurrence of old_string. Default false replaces only the first unique match.",
        )),
    );
    props.insert(
        "old_string".to_string(),
        string_prop(
            Some(1),
            None,
            Some("Exact text to find in the file. Copy verbatim from workspace__readFile output, including whitespace and newlines."),
        ),
    );
    props.insert(
        "new_string".to_string(),
        string_prop(
            Some(0),
            None,
            Some("Replacement text. Use an empty string to delete the matched block."),
        ),
    );

    MCPTool {
        name: "strReplace".to_string(),
        title: Some("Replace Text in File".to_string()),
        description: "Perform exact string replacement in an existing file.

PREREQUISITE: Use workspace__readFile first and copy the exact text block into old_string. Matching is literal — whitespace, indentation, and line endings must match.

- Single replacement (default): old_string must match exactly once unless replace_all=true.
- replace_all=true: every occurrence of old_string is replaced.
- new_string may be empty to delete the matched text.

Use workspace__writeFile mode='create' for new files and mode='overwrite' only when replacing the entire file."
            .to_string(),
        input_schema: object_schema(
            props,
            vec![
                "path".to_string(),
                "old_string".to_string(),
                "new_string".to_string(),
            ],
        ),
        output_schema: None,
        annotations: None,
    }
}

#[cfg(feature = "workspace-edit-file")]
/// Maximum number of edit operations allowed in a legacy batch `edits` array.
pub const EDIT_FILE_MAX_EDITS: u32 = 50;

#[cfg(feature = "workspace-edit-file")]
fn edit_file_path_prop() -> JSONSchema {
    string_prop(
        Some(1),
        Some(1000),
        Some("Path to the file to edit. Relative paths resolve from the workspace; absolute paths are also allowed unless protected. Use @teamwork/... or .libragent/teamwork/... to edit teamwork files without changing workspaceOverride."),
    )
}

#[cfg(feature = "workspace-edit-file")]
fn edit_start_prop(include_zero: bool) -> JSONSchema {
    let description = if include_zero {
        "Copy the '42:a31f2c' from workspace__readFile's '42:a31f2c|content' line format. Omit only the trailing '|content'. Use \"0\" only to prepend at the file top."
    } else {
        "Copy the '42:a31f2c' from workspace__readFile's '42:a31f2c|content' line format. Omit only the trailing '|content'."
    };
    string_prop(Some(1), Some(32), Some(description))
}

#[cfg(feature = "workspace-edit-file")]
fn edit_end_prop() -> JSONSchema {
    string_prop(
        Some(1),
        Some(32),
        Some(
            "Copy the end line's '72:b47aa1' from workspace__readFile's '72:b47aa1|content' format for multi-line ranges. Omit for single-line edits.",
        ),
    )
}

#[cfg(feature = "workspace-edit-file")]
fn edit_anchor_props() -> SchemaProperties {
    let start_anchor_desc =
        "Legacy: 6-character opaque anchor for the start line. Prefer start: \"N:anchor\" instead.";
    let end_anchor_desc =
        "Legacy: 6-character opaque anchor for the end line. Prefer end: \"N:anchor\" instead.";

    let mut props = SchemaProperties::new();
    props.insert(
        "anchor".to_string(),
        string_prop(None, None, Some(start_anchor_desc)),
    );
    props.insert(
        "startAnchor".to_string(),
        string_prop(None, None, Some(start_anchor_desc)),
    );
    props.insert(
        "endAnchor".to_string(),
        string_prop(None, None, Some(end_anchor_desc)),
    );
    props
}

#[cfg(feature = "workspace-edit-file")]
fn create_prepend_flat_variant() -> JSONSchema {
    let mut props = SchemaProperties::new();
    props.insert("path".to_string(), edit_file_path_prop());
    props.insert(
        "start".to_string(),
        string_const_prop(
            "0",
            Some("Optional. Must be \"0\" when provided for prepend."),
        ),
    );
    props.insert(
        "content".to_string(),
        string_prop(
            Some(0),
            None,
            Some("Content to insert at the beginning of the file."),
        ),
    );

    let mut schema = object_schema(props, vec!["path".to_string(), "content".to_string()]);
    schema.description = Some(
        "Prepend content at the top of the file. Provide path + content (optionally start: \"0\")."
            .to_string(),
    );
    schema
}

#[cfg(feature = "workspace-edit-file")]
fn create_line_edit_flat_variant() -> JSONSchema {
    let mut props = SchemaProperties::new();
    props.insert("path".to_string(), edit_file_path_prop());
    props.insert("start".to_string(), edit_start_prop(false));
    props.insert("end".to_string(), edit_end_prop());
    props.insert(
        "op".to_string(),
        enum_prop_optional(
            vec!["replace", "delete"],
            Some("Optional hint for replace or delete. Omit to let the server infer from content."),
        ),
    );
    props.insert(
        "content".to_string(),
        string_prop(
            None,
            None,
            Some(
                "Replacement content. Omit to delete the targeted line or range. The server infers replace vs delete from content presence when op is omitted.",
            ),
        ),
    );

    let mut schema = object_schema(props, vec!["path".to_string(), "start".to_string()]);
    schema.description = Some(
        "Replace or delete existing lines. Copy start (and optional end) as \"N:anchor\" from workspace__readFile."
            .to_string(),
    );
    schema
}

#[cfg(feature = "workspace-edit-file")]
fn create_insert_after_flat_variant() -> JSONSchema {
    let mut props = SchemaProperties::new();
    props.insert("path".to_string(), edit_file_path_prop());
    props.insert(
        "op".to_string(),
        string_const_prop(
            "insert_after",
            Some("Must be insert_after for this edit shape."),
        ),
    );
    props.insert("start".to_string(), edit_start_prop(true));
    props.insert(
        "content".to_string(),
        string_prop(
            Some(0),
            None,
            Some("Content to insert after the anchored line (or at top when start is \"0\")."),
        ),
    );

    let mut schema = object_schema(
        props,
        vec![
            "path".to_string(),
            "op".to_string(),
            "start".to_string(),
            "content".to_string(),
        ],
    );
    schema.description =
        Some("Insert content after an existing line (or prepend when start is \"0\").".to_string());
    schema
}

#[cfg(feature = "workspace-edit-file")]
fn create_prepend_edit_item_variant() -> JSONSchema {
    let mut props = SchemaProperties::new();
    props.insert(
        "startLine".to_string(),
        integer_const_prop(
            0,
            Some("Must be 0 for prepend. Omit startLine to prepend with content only."),
        ),
    );
    props.insert(
        "content".to_string(),
        string_prop(
            Some(0),
            None,
            Some("Content to insert at the beginning of the file."),
        ),
    );

    let mut schema = object_schema(props, vec!["content".to_string()]);
    schema.description = Some(
        "Prepend content at the top of the file. Provide content only, or content with startLine: 0."
            .to_string(),
    );
    schema
}

#[cfg(feature = "workspace-edit-file")]
fn create_line_edit_item_variant() -> JSONSchema {
    let mut props = SchemaProperties::new();
    props.insert(
        "startLine".to_string(),
        integer_prop(
            Some(1),
            None,
            Some(
                "Target start line number (1-based). Prefer flat start: \"N:anchor\" on workspace__editFile.",
            ),
        ),
    );
    props.insert(
        "endLine".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("Inclusive end line for a multi-line replace/delete range."),
        ),
    );
    props.insert(
        "op".to_string(),
        enum_prop_optional(
            vec!["replace", "delete"],
            Some("Optional hint for replace or delete."),
        ),
    );
    for (key, value) in edit_anchor_props() {
        props.insert(key, value);
    }
    props.insert(
        "content".to_string(),
        string_prop(
            None,
            None,
            Some("Replacement content. Omit to delete the targeted line or range."),
        ),
    );

    let mut schema = object_schema(props, vec!["startLine".to_string()]);
    schema.description = Some(
        "Replace or delete existing lines. Requires startLine plus anchor for existing-line edits."
            .to_string(),
    );
    schema
}

#[cfg(feature = "workspace-edit-file")]
fn create_insert_after_edit_item_variant() -> JSONSchema {
    let mut props = SchemaProperties::new();
    props.insert(
        "op".to_string(),
        string_const_prop(
            "insert_after",
            Some("Must be insert_after for this edit shape."),
        ),
    );
    props.insert(
        "startLine".to_string(),
        integer_prop(
            Some(0),
            None,
            Some("Line number to insert after (1-based). Use 0 only to prepend at the file top."),
        ),
    );
    for (key, value) in edit_anchor_props() {
        props.insert(key, value);
    }
    props.insert(
        "content".to_string(),
        string_prop(
            Some(0),
            None,
            Some("Content to insert after the anchored line."),
        ),
    );

    let mut schema = object_schema(
        props,
        vec![
            "op".to_string(),
            "startLine".to_string(),
            "content".to_string(),
        ],
    );
    schema.description =
        Some("Insert content after an existing line (or prepend when startLine is 0).".to_string());
    schema
}

#[cfg(feature = "workspace-edit-file")]
/// Internal/legacy item schema used after flat args are wrapped into `edits: [one]`.
pub fn create_edit_item_schema() -> JSONSchema {
    one_of_object_schema(
        vec![
            create_prepend_edit_item_variant(),
            create_line_edit_item_variant(),
            create_insert_after_edit_item_variant(),
        ],
        Some(
            "A single edit operation after canonicalization. Prefer the flat workspace__editFile discovery schema with start/end.",
        ),
    )
}

#[cfg(feature = "workspace-edit-file")]
/// Model-facing schema: one flat edit object (path + start/end/content).
pub fn create_edit_file_input_schema() -> JSONSchema {
    one_of_object_schema(
        vec![
            create_prepend_flat_variant(),
            create_line_edit_flat_variant(),
            create_insert_after_flat_variant(),
        ],
        Some(
            "Edit one location in a file. Copy start/end as \"N:anchor\" from workspace__readFile(showLineAnchors=true).",
        ),
    )
}

#[cfg(feature = "workspace-edit-file")]
/// Runtime validation schema after flat calls are wrapped into `{ path, edits: [...] }`.
pub fn create_edit_file_validation_schema() -> JSONSchema {
    let mut props = SchemaProperties::new();
    props.insert("path".to_string(), edit_file_path_prop());
    props.insert(
        "edits".to_string(),
        array_schema_with_max_items(
            create_edit_item_schema(),
            Some(EDIT_FILE_MAX_EDITS),
            Some("Internal edit list after canonicalization. Model-facing workspace__editFile uses a single flat object."),
        ),
    );

    object_schema(props, vec!["path".to_string(), "edits".to_string()])
}

#[cfg(feature = "workspace-edit-file")]
pub fn create_edit_file_tool() -> MCPTool {
    MCPTool {
        name: "editFile".to_string(),
        title: Some("Edit File".to_string()),
        description: "Apply one line edit to a file.

PREREQUISITE: Obtain anchors from workspace__readFile(showLineAnchors=true), workspace__writeFile, or a previous workspace__editFile response. Anchored lines look like `42:a31f2c|...` — copy the `42:a31f2c` prefix into start (and end for ranges).

Shapes:
- Prepend: `{ \"path\": \"a.rs\", \"content\": \"...\" }`
- Replace: `{ \"path\": \"a.rs\", \"start\": \"10:a31f2c\", \"content\": \"...\" }`
- Delete: `{ \"path\": \"a.rs\", \"start\": \"10:a31f2c\" }`
- Range replace: `{ \"path\": \"a.rs\", \"start\": \"10:a31f2c\", \"end\": \"15:b47aa1\", \"content\": \"...\" }`
- Insert after: `{ \"path\": \"a.rs\", \"op\": \"insert_after\", \"start\": \"10:a31f2c\", \"content\": \"...\" }`

One edit per call. For multiple locations, call workspace__editFile again (re-read when line numbers may have shifted)."
            .to_string(),
        input_schema: create_edit_file_input_schema(),
        output_schema: None,
        annotations: None,
    }
}
