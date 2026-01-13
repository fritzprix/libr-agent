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

    MCPTool {
        name: "readFile".to_string(),
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
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path to the file to write (from workspace root)"),
        ),
    );
    props.insert(
        "content".to_string(),
        string_prop(
            None,
            None,
            Some("Content to write to the file. Actual maximum is enforced server-side via LIBRAGENT_MAX_FILE_SIZE"),
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
        name: "writeFile".to_string(),
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
            Some("Relative path to the directory to list (from workspace root)"),
        ),
    );

    MCPTool {
        name: "listDirectory".to_string(),
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
        "srcAbsPath".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Absolute path of source file to import"),
        ),
    );
    props.insert(
        "destRelPath".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path in workspace where file will be imported"),
        ),
    );

    MCPTool {
        name: "importFile".to_string(),
        title: Some("Import File".to_string()),
        description: "Import an external file into the workspace".to_string(),
        input_schema: object_schema(
            props,
            vec!["srcAbsPath".to_string(), "destRelPath".to_string()],
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_replace_string_in_file_tool() -> MCPTool {
    let mut item_props = HashMap::new();
    item_props.insert(
        "oldString".to_string(),
        string_prop(
            None,
            None,
            Some("Exact text content to find and replace. Must match precisely including whitespace. Include surrounding context (3-5 lines) for uniqueness."),
        ),
    );
    item_props.insert(
        "newString".to_string(),
        string_prop(
            None,
            None,
            Some("New text content to replace oldString with. Use empty string to delete the matched text."),
        ),
    );

    let replacement_item_schema = object_schema(
        item_props,
        vec!["oldString".to_string(), "newString".to_string()],
    );

    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Relative path to the file to modify (from workspace root)"),
        ),
    );
    props.insert(
        "replacements".to_string(),
        array_schema(
            replacement_item_schema,
            Some("An array of string replacement objects"),
        ),
    );

    MCPTool {
        name: "replaceStringInFile".to_string(),
        title: Some("Replace String in File".to_string()),
        description: "Replace text content in a file using exact string matching. More robust than line-based replacement as it works regardless of line number changes. Supports multiple independent replacements in a single call.".to_string(),
        input_schema: object_schema(props, vec!["path".to_string(), "replacements".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
