use crate::mcp::{schema::SchemaProperties, utils::schema_builder::*, MCPTool};

use crate::mcp::builtin::workspace::edit_mode::PRIMARY_EDIT_TOOL;

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
        "Write mode. 'create' (default) writes a new file; if the path already exists it keeps that file and writes to a sibling path with a numeric suffix (e.g. report-1.md) instead of failing. 'overwrite' is for entire-file replacement only—do not use it for a few-line change (use {PRIMARY_EDIT_TOOL}). 'append' adds content verbatim to the end (no automatic newline)."
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
            Some("File content to write. Empty string creates an empty file. In append mode, content is written verbatim—prefix with \\n when adding after an existing line. For overwrite, pass the complete new file contents."),
        ),
    );

    MCPTool {
        name: "writeFile".to_string(),
        title: Some("Write File".to_string()),
        description: format!(
            "Create, overwrite, or append content to a file. Missing parent directories are created automatically. Default mode='create': if the target already exists, content is saved to a new sibling path (stem-N.ext) and the response clearly reports the alternate path—existing files are never overwritten unless mode='overwrite'. mode='overwrite' replaces the entire file and returns a change summary/diff—use it only for full-file replacement (codegen, scaffold, formatter output); for ≤ a few hunks use {PRIMARY_EDIT_TOOL}. Append writes content verbatim—include \\n in content when starting a new line."
        ),
        input_schema: object_schema(props, vec!["path".to_string(), "content".to_string()]),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
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
        libragent_wait: None,
    }
}
