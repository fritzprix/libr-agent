
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
        string_prop(
            Some(1),
            Some(1000),
            Some("Path to the directory to list"),
        ),
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

pub fn create_search_files_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "pattern".to_string(),
        string_prop(
            Some(1),
            Some(500),
            Some("Glob pattern to match files (e.g., '*.rs', '**/*.tsx')"),
        ),
    );
    props.insert(
        "path".to_string(),
        string_prop(Some(1), Some(1000), Some("Root path to search from")),
    );
    props.insert(
        "max_depth".to_string(),
        integer_prop(
            Some(1),
            Some(50),
            Some("Maximum depth to search (optional)"),
        ),
    );
    props.insert(
        "file_type".to_string(),
        string_prop(
            None,
            None,
            Some("Filter by file type: 'file', 'dir', or 'both'"),
        ),
    );

    MCPTool {
        name: "search_files".to_string(),
        title: Some("Search Files".to_string()),
        description: "Search for files matching patterns with various filters".to_string(),
        input_schema: object_schema(props, vec!["pattern".to_string()]),
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
        "content".to_string(),
        string_prop(None, None, Some("The new content for the line range")),
    );

    // 기존 line_number 지원을 위한 backward compatibility
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
        vec!["start_line".to_string(), "content".to_string()],
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
        description: "Replace specific lines or line ranges in a file with new content".to_string(),
        input_schema: object_schema(
            props,
            vec!["path".to_string(), "replacements".to_string()],
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_grep_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "pattern".to_string(),
        string_prop(Some(1), None, Some("Regex pattern to search for")),
    );
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Path to the file to search (exclusive with 'input')"),
        ),
    );
    props.insert(
        "input".to_string(),
        string_prop(
            Some(1),
            None,
            Some("Input string to search (exclusive with 'path')"),
        ),
    );
    props.insert(
        "ignore_case".to_string(),
        boolean_prop(Some("Perform case-insensitive matching")),
    );
    props.insert(
        "line_numbers".to_string(),
        boolean_prop(Some("Include line numbers in the output")),
    );

    MCPTool {
        name: "grep".to_string(),
        title: Some("Grep".to_string()),
        description: "Search for a pattern in a file or input string.".to_string(),
        input_schema: object_schema(props, vec!["pattern".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
