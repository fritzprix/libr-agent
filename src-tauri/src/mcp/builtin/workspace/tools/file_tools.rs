use crate::mcp::{schema::SchemaProperties, utils::schema_builder::*, MCPTool};

#[cfg(feature = "workspace-edit-file")]
use crate::mcp::schema::JSONSchema;

use super::super::edit_mode::{read_file_tool_hint, PRIMARY_EDIT_TOOL};

#[cfg(all(
    feature = "workspace-edit-file",
    not(feature = "workspace-str-replace")
))]
use super::super::edit_mode::{
    read_file_show_line_anchors_schema_hint, search_show_line_anchors_schema_hint,
};

// Note: maximum file size is enforced at runtime (LIBRAGENT_MAX_FILE_SIZE).
// The input schema cannot call runtime functions; therefore `content` has no hard cap here.

pub fn create_read_file_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Path to the file to read. Relative paths resolve from the workspace; absolute paths are also allowed unless protected. Use @teamwork/... or .libragent/teamwork/... for the canonical teamwork scaffold root. Read-only skill aliases are also available: @system-skills/..., @user-skills/..., @assistant-skills/..., and @workspace-skills/... when those roots exist for the session."),
        ),
    );

    props.insert(
        "offset".to_string(),
        integer_prop(
            None,
            None,
            Some("Starting line index (1-based or 0-based; both 0 and 1 start at the first line). Alias to startLine. Can be negative in tail mode to skip from the end (e.g. -100)."),
        ),
    );
    props.insert(
        "size".to_string(),
        integer_prop(
            None,
            None,
            Some("Number of lines to read. If negative, reads that many lines from the end of the file (tail mode)."),
        ),
    );
    #[cfg(all(
        feature = "workspace-edit-file",
        not(feature = "workspace-str-replace")
    ))]
    props.insert(
        "showLineAnchors".to_string(),
        boolean_prop(Some(read_file_show_line_anchors_schema_hint())),
    );

    MCPTool {
        name: "readFile".to_string(),
        title: Some("Read File".to_string()),
        description: format!(
            "Read the contents of a file. Supports reading from a specific offset and line count (size), including negative size for tailing the end of the file. Large responses are chunked automatically to stay inline. {}",
            read_file_tool_hint()
        ),
        input_schema: object_schema(props, vec!["path".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_write_file_tool() -> MCPTool {
    // Field order is intentional: path → mode → content.
    // Models often emit arguments in schema order; putting mode before the long
    // content payload reduces accidental omission of mode after drafting content.
    let mut props = SchemaProperties::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Path to write. Relative paths resolve from the workspace; absolute paths are also allowed unless protected. Use @teamwork/... or .libragent/teamwork/... to write into the canonical teamwork scaffold root without changing workspaceOverride."),
        ),
    );
    let mode_description = format!(
        "Write mode. 'create' (default) writes a new file; if the path already exists it keeps that file and writes to a sibling path with a numeric suffix (e.g. report-1.md) instead of failing. 'overwrite' replaces the entire existing file. 'append' adds content verbatim to the end (no automatic newline). Use overwrite/append/{PRIMARY_EDIT_TOOL} when you intend to change an existing file."
    );
    props.insert(
        "mode".to_string(),
        enum_prop(
            vec!["create", "overwrite", "append"],
            "create",
            Some(mode_description.as_str()),
        ),
    );
    props.insert(
        "content".to_string(),
        string_prop(
            None,
            None,
            Some("File content to write. Empty string creates an empty file. In append mode, content is written verbatim—prefix with \\n when adding after an existing line."),
        ),
    );

    MCPTool {
        name: "writeFile".to_string(),
        title: Some("Write File".to_string()),
        description: format!(
            "Create, overwrite, or append content to a file. Missing parent directories are created automatically. Default mode='create': if the target already exists, content is saved to a new sibling path (stem-N.ext) and the response clearly reports the alternate path—existing files are never overwritten unless mode='overwrite'. Append writes content verbatim—include \\n in content when starting a new line. Use {PRIMARY_EDIT_TOOL} for targeted in-place edits. mode='overwrite' returns a diff of the changes."
        ),
        input_schema: object_schema(props, vec!["path".to_string(), "content".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_list_directory_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Path to the directory to list. Relative paths resolve from the workspace; absolute paths are also allowed unless protected. Use @teamwork or @teamwork/... (or relative .libragent/teamwork/...) for the canonical teamwork scaffold root. Read-only skill aliases such as @system-skills or @user-skills may also be listed when available."),
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

Returns names and types (file/directory). Use globFiles when you need glob-style filtering."
            .to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_import_files_tool() -> MCPTool {
    let mut file_item_props = SchemaProperties::new();
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

    let mut props = SchemaProperties::new();
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

fn search_path_prop() -> crate::mcp::schema::JSONSchema {
    string_prop(
        Some(1),
        Some(1000),
        Some("Path to the file or directory to search. Relative paths resolve from the workspace; absolute paths are also allowed unless protected. Use @teamwork or @teamwork/... (or relative .libragent/teamwork/...) for the canonical teamwork scaffold root. Read-only skill aliases such as @system-skills/... and @user-skills/... may also be searched when available."),
    )
}

pub fn create_glob_files_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert("path".to_string(), search_path_prop());
    props.insert(
        "filePattern".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Glob pattern to match file or directory names (e.g. '*.rs', 'src/**/*.ts')."),
        ),
    );
    props.insert(
        "limit".to_string(),
        integer_prop(
            Some(1),
            Some(1000),
            Some("Maximum number of matched files/directories to return (default: 50)."),
        ),
    );
    props.insert(
        "offset".to_string(),
        integer_prop(
            Some(0),
            None,
            Some("Number of results to skip for pagination (default: 0)."),
        ),
    );

    MCPTool {
        name: "globFiles".to_string(),
        title: Some("Glob Workspace Files".to_string()),
        description: "Find files and directories by glob pattern. Use grepFiles to search inside matches, or readFile to inspect a specific path.".to_string(),
        input_schema: object_schema(
            props,
            vec!["path".to_string(), "filePattern".to_string()],
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_grep_files_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert("path".to_string(), search_path_prop());
    props.insert(
        "query".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Regular expression pattern to search for text inside files. Matched against full file content with multiline mode enabled, so ^ and $ match line boundaries. '.' does not match newlines unless you opt into that in the regex itself (for example with (?s))."),
        ),
    );
    props.insert(
        "filePattern".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Optional glob pattern to limit which files are searched (e.g. '*.rs', 'src/**/*.ts')."),
        ),
    );
    props.insert(
        "limit".to_string(),
        integer_prop(
            Some(1),
            Some(1000),
            Some("Maximum number of matching lines to return (default: 50)."),
        ),
    );
    props.insert(
        "offset".to_string(),
        integer_prop(
            Some(0),
            None,
            Some("Number of matching lines to skip for pagination (default: 0)."),
        ),
    );
    props.insert(
        "ignoreCase".to_string(),
        boolean_prop(Some("Case-insensitive search (default: false).")),
    );
    #[cfg(all(
        feature = "workspace-edit-file",
        not(feature = "workspace-str-replace")
    ))]
    props.insert(
        "showLineAnchors".to_string(),
        boolean_prop(Some(search_show_line_anchors_schema_hint())),
    );

    MCPTool {
        name: "grepFiles".to_string(),
        title: Some("Grep Workspace Files".to_string()),
        description: format!(
            "Search file contents with a regex pattern. Results are line-based and paginated by matching lines. Use readFile on a hit, then {PRIMARY_EDIT_TOOL} to apply targeted edits."
        ),
        input_schema: object_schema(props, vec!["path".to_string(), "query".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Legacy combined search tool kept for backward-compatible dispatch only.
pub fn create_search_tool() -> MCPTool {
    let mut props = SchemaProperties::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Path to the file or directory to search. Relative paths resolve from the workspace; absolute paths are also allowed unless protected. Use @teamwork or @teamwork/... (or relative .libragent/teamwork/...) for the canonical teamwork scaffold root. Read-only skill aliases such as @system-skills/... and @user-skills/... may also be searched when available."),
        ),
    );
    props.insert(
        "limit".to_string(),
        integer_prop(
            Some(1),
            Some(1000),
            Some("Maximum number of results to return (default: 50). For file-name search this limits matched files/directories; for content search this limits matching lines after regex expansion."),
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
    #[cfg(all(
        feature = "workspace-edit-file",
        not(feature = "workspace-str-replace")
    ))]
    props.insert(
        "showLineAnchors".to_string(),
        boolean_prop(Some(search_show_line_anchors_schema_hint())),
    );

    MCPTool {
        name: "searchFiles".to_string(),
        title: Some("Search Workspace (Deprecated)".to_string()),
        description: "DEPRECATED: Use globFiles for filename search or grepFiles for content search. \
                     This tool is kept for backward compatibility and will be removed in a future version. \
                     Note: Requires either 'query' (content search) or 'filePattern' (filename search).".to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

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
                "Required when afterLine targets an existing line. Use only the 6-character anchor from readFile(showLineAnchors=true) or search(showLineAnchors=true). Omit only when afterLine is 0.",
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

// Note: maximum file size is enforced at runtime (LIBRAGENT_MAX_FILE_SIZE).
// The input schema cannot call runtime functions; therefore `content` has no hard cap here.

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
            Some("Exact text to find in the file. Copy verbatim from readFile output, including whitespace and newlines."),
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

PREREQUISITE: Use readFile first and copy the exact text block into old_string. Matching is literal — whitespace, indentation, and line endings must match.

- Single replacement (default): old_string must match exactly once unless replace_all=true.
- replace_all=true: every occurrence of old_string is replaced.
- new_string may be empty to delete the matched text.

Use writeFile mode='create' for new files and mode='overwrite' only when replacing the entire file."
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
/// Maximum number of edit operations allowed in a single editFile call.
pub const EDIT_FILE_MAX_EDITS: u32 = 50;

#[cfg(feature = "workspace-edit-file")]
fn edit_anchor_props() -> SchemaProperties {
    let start_anchor_desc =
        "6-character opaque anchor for the start line from readFile(showLineAnchors=true). Required for edits that touch an existing line. Do not include the line number or '|content'.";
    let end_anchor_desc =
        "6-character opaque anchor for the end line. Required when endLine is set for a multi-line replace/delete range.";

    let mut props = SchemaProperties::new();
    props.insert(
        "anchor".to_string(),
        string_prop(None, None, Some(start_anchor_desc)),
    );
    props.insert(
        "startAnchor".to_string(),
        string_prop(
            None,
            None,
            Some("Alias for anchor. Prefer anchor for new edits."),
        ),
    );
    props.insert(
        "endAnchor".to_string(),
        string_prop(None, None, Some(end_anchor_desc)),
    );
    props
}

#[cfg(feature = "workspace-edit-file")]
fn create_prepend_edit_variant() -> JSONSchema {
    let content_desc =
        "Content to insert at the beginning of the file. startLine defaults to 0 when omitted.";

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
        string_prop(Some(0), None, Some(content_desc)),
    );

    let mut schema = object_schema(props, vec!["content".to_string()]);
    schema.description = Some(
        "Prepend content at the top of the file. Provide content only, or content with startLine: 0. Do not include anchors."
            .to_string(),
    );
    schema
}

#[cfg(feature = "workspace-edit-file")]
fn create_line_edit_variant() -> JSONSchema {
    let start_line_desc = "Target start line number (1-based). Use endLine only for multi-line replace/delete ranges.";
    let end_line_desc =
        "Inclusive end line for a multi-line replace/delete range. Omit for a single-line edit.";
    let content_desc =
        "Replacement content. Omit to delete the targeted line or range. The server infers replace vs delete from content presence when op is omitted.";

    let mut props = SchemaProperties::new();
    props.insert(
        "startLine".to_string(),
        integer_prop(Some(1), None, Some(start_line_desc)),
    );
    props.insert(
        "endLine".to_string(),
        integer_prop(Some(1), None, Some(end_line_desc)),
    );
    props.insert(
        "op".to_string(),
        enum_prop_optional(
            vec!["replace", "delete"],
            Some("Optional hint for replace or delete. Omit to let the server infer from content."),
        ),
    );
    for (key, value) in edit_anchor_props() {
        props.insert(key, value);
    }
    props.insert(
        "content".to_string(),
        string_prop(None, None, Some(content_desc)),
    );

    let mut schema = object_schema(props, vec!["startLine".to_string()]);
    schema.description = Some(
        "Replace or delete existing lines. Requires startLine plus anchor for existing-line edits."
            .to_string(),
    );
    schema
}

#[cfg(feature = "workspace-edit-file")]
fn create_insert_after_edit_variant() -> JSONSchema {
    let start_line_desc =
        "Line number to insert after (1-based). Use 0 only to prepend at the file top.";
    let content_desc = "Content to insert after the anchored line.";

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
        integer_prop(Some(0), None, Some(start_line_desc)),
    );
    for (key, value) in edit_anchor_props() {
        props.insert(key, value);
    }
    props.insert(
        "content".to_string(),
        string_prop(Some(0), None, Some(content_desc)),
    );

    let mut schema = object_schema(
        props,
        vec![
            "op".to_string(),
            "startLine".to_string(),
            "content".to_string(),
        ],
    );
    schema.description = Some(
        "Insert content after an existing line (or prepend when startLine is 0). Requires anchor unless startLine is 0."
            .to_string(),
    );
    schema
}

#[cfg(feature = "workspace-edit-file")]
pub fn create_edit_item_schema() -> JSONSchema {
    one_of_object_schema(
        vec![
            create_prepend_edit_variant(),
            create_line_edit_variant(),
            create_insert_after_edit_variant(),
        ],
        Some(
            "A single edit operation. Choose the variant that matches the intent: prepend, replace/delete existing lines, or insert_after.",
        ),
    )
}

#[cfg(feature = "workspace-edit-file")]
pub fn create_edit_file_input_schema() -> JSONSchema {
    let mut props = SchemaProperties::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Path to the file to edit. Relative paths resolve from the workspace; absolute paths are also allowed unless protected. Use @teamwork/... or .libragent/teamwork/... to edit teamwork files without changing workspaceOverride."),
        ),
    );
    props.insert(
        "edits".to_string(),
        array_schema_with_max_items(
            create_edit_item_schema(),
            Some(EDIT_FILE_MAX_EDITS),
            Some("Ordered list of edit operations to apply atomically to one file. All edits are schema-validated and anchor-validated before any write. Edits must not overlap."),
        ),
    );

    object_schema(props, vec!["path".to_string(), "edits".to_string()])
}

#[cfg(feature = "workspace-edit-file")]
pub fn create_edit_file_tool() -> MCPTool {
    MCPTool {
        name: "editFile".to_string(),
        title: Some("Edit File (Batch)".to_string()),
        description: "Apply multiple line edits to one file atomically in a single operation.

PREREQUISITE: Obtain anchors from a prior readFile(showLineAnchors=true), writeFile response, or previous editFile response. Anchored lines look like `42:a31f2c|...`; for anchors, pass only the 6-character code such as `a31f2c`, not `42:a31f2c`.

Edit shapes (one per array item):
- Prepend: `{ \"content\": \"...\" }` or `{ \"startLine\": 0, \"content\": \"...\" }`
- Replace/delete existing lines: `{ \"startLine\": N, \"anchor\": \"...\", \"content\": \"...\" }` (omit content to delete)
- Insert after a line: `{ \"op\": \"insert_after\", \"startLine\": N, \"anchor\": \"...\", \"content\": \"...\" }`

Line numbering is 1-based for existing content. The only valid 0 value is startLine=0, which prepends at the file top.

All edits are validated before any write begins."
            .to_string(),
        input_schema: create_edit_file_input_schema(),
        output_schema: None,
        annotations: None,
    }
}
