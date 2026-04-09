use crate::mcp::{utils::schema_builder::*, MCPTool};
use std::collections::HashMap;

pub fn create_export_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "paths".to_string(),
        array_schema(
            string_prop(Some(1), Some(1000), None),
            Some("Array of relative file or directory paths to export (from workspace root)"),
        ),
    );
    props.insert(
        "name".to_string(),
        string_prop(
            None,
            Some(50),
            Some("Display name or package name (optional). If omitted, a name is auto-generated."),
        ),
    );

    MCPTool {
        name: "export".to_string(),
        title: Some("Export Files".to_string()),
        description: "Export one or more workspace files/directories and return a downloadable HTML resource.\n\
                      - A single file is exported as-is.\n\
                      - Multiple paths or any directory are packaged as a ZIP."
            .to_string(),
        input_schema: object_schema(props, vec!["paths".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
