use crate::mcp::{utils::schema_builder::*, MCPTool};

use std::collections::HashMap;

use super::super::utils::constants::MAX_FILE_SIZE;

pub fn create_read_file_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(Some(1), Some(1000), Some("Path to the file to read")),
    );
    props.insert(
        "start_line".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("Starting line number (1-based, optional)"),
        ),
    );
    props.insert(
        "end_line".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("Ending line number (1-based, optional)"),
        ),
    );

    MCPTool {
        name: "read_file".to_string(),
        title: Some("Read File".to_string()),
        description: "Read the contents of a file, optionally specifying line ranges".to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_write_file_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(Some(1), Some(1000), Some("Path to the file to write")),
    );
    props.insert(
        "content".to_string(),
        string_prop(
            None,
            Some(MAX_FILE_SIZE as u32),
            Some("Content to write to the file"),
        ),
    );
    props.insert(
        "mode".to_string(),
        string_prop(
            None,
            None,
            Some("Write mode: 'w' for overwrite (default), 'a' for append"),
        ),
    );

    MCPTool {
        name: "write_file".to_string(),
        title: Some("Write File".to_string()),
        description: "Write content to a file with optional append mode".to_string(),
        input_schema: object_schema(props, vec!["path".to_string(), "content".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_list_directory_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(Some(1), Some(1000), Some("Path to the directory to list")),
    );

    MCPTool {
        name: "list_directory".to_string(),
        title: Some("List Directory".to_string()),
        description: "List contents of a directory".to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_import_file_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "src_abs_path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Absolute path of source file to import"),
        ),
    );
    props.insert(
        "dest_rel_path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path in workspace where file will be imported"),
        ),
    );

    MCPTool {
        name: "import_file".to_string(),
        title: Some("Import File".to_string()),
        description: "Import an external file into the workspace".to_string(),
        input_schema: object_schema(
            props,
            vec!["src_abs_path".to_string(), "dest_rel_path".to_string()],
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_replace_lines_in_file_tool() -> MCPTool {
    let mut item_props = HashMap::new();
    item_props.insert(
        "start_line".to_string(),
        integer_prop(Some(1), None, Some("Starting line number (1-based)")),
    );
    item_props.insert(
        "end_line".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("Ending line number (1-based, optional). If not provided, equals start_line"),
        ),
    );
    item_props.insert(
        "new_content".to_string(),
        string_prop(
            None,
            None,
            Some("The new content for the line range. Use empty string to delete lines."),
        ),
    );

    // Backward compatibility for existing line_number support
    item_props.insert(
        "line_number".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("The 1-based line number to replace (deprecated, use start_line)"),
        ),
    );

    let replacement_item_schema = object_schema(
        item_props,
        vec!["start_line".to_string()], // new_content is now optional for line deletion
    );

    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(Some(1), Some(1000), Some("Path to the file to modify")),
    );
    props.insert(
        "replacements".to_string(),
        array_schema(
            replacement_item_schema,
            Some("An array of line replacement objects"),
        ),
    );

    MCPTool {
        name: "replace_lines_in_file".to_string(),
        title: Some("Replace Lines in File".to_string()),
        description: "Replace specific lines or line ranges in a file with new content. Use empty content string to delete lines.".to_string(),
        input_schema: object_schema(props, vec!["path".to_string(), "replacements".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
