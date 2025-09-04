
use crate::mcp::{utils::schema_builder::*, MCPTool};
use std::collections::HashMap;

pub fn create_export_file_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "path".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Workspace 내 export할 파일 경로"),
        ),
    );
    props.insert(
        "display_name".to_string(),
        string_prop(None, None, Some("다운로드시 표시할 파일명 (선택적)")),
    );
    props.insert(
        "description".to_string(),
        string_prop(None, None, Some("파일 설명 (선택적)")),
    );

    MCPTool {
        name: "export_file".to_string(),
        title: Some("Export Single File".to_string()),
        description: "Export a single file from workspace for download with interactive UI"
            .to_string(),
        input_schema: object_schema(props, vec!["path".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_export_zip_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "files".to_string(),
        array_schema(
            string_prop(Some(1), Some(1000), None),
            Some("Export할 파일 경로들의 배열"),
        ),
    );
    props.insert(
        "package_name".to_string(),
        string_prop(
            None,
            Some(50),
            Some("ZIP 패키지명 (선택적, 기본값: workspace_export)"),
        ),
    );
    props.insert(
        "description".to_string(),
        string_prop(None, None, Some("패키지 설명 (선택적)")),
    );

    MCPTool {
        name: "export_zip".to_string(),
        title: Some("Export ZIP Package".to_string()),
        description: "Export multiple files as a ZIP package for download with interactive UI"
            .to_string(),
        input_schema: object_schema(props, vec!["files".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
