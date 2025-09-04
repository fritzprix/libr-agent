use crate::mcp::{utils::schema_builder::*, MCPTool};
use serde_json::json;
use std::collections::HashMap;

use super::super::utils::constants::{
    DEFAULT_EXECUTION_TIMEOUT, MAX_CODE_SIZE, MAX_EXECUTION_TIMEOUT,
};

pub fn create_execute_python_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "code".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(MAX_CODE_SIZE as u32),
            Some("Python code to execute"),
            vec![json!("print('Hello, World!')")],
        ),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(1),
            Some(MAX_EXECUTION_TIMEOUT as i64),
            DEFAULT_EXECUTION_TIMEOUT as i64,
            Some("Timeout in seconds (default: 30)"),
        ),
    );

    MCPTool {
        name: "execute_python".to_string(),
        title: Some("Execute Python Code".to_string()),
        description: "Execute Python code in a sandboxed environment".to_string(),
        input_schema: object_schema(props, vec!["code".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_execute_typescript_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "code".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(MAX_CODE_SIZE as u32),
            Some("TypeScript code to execute"),
            vec![json!("console.log('Hello, World!');")],
        ),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(1),
            Some(MAX_EXECUTION_TIMEOUT as i64),
            DEFAULT_EXECUTION_TIMEOUT as i64,
            Some("Timeout in seconds (default: 30)"),
        ),
    );

    MCPTool {
        name: "execute_typescript".to_string(),
        title: Some("Execute TypeScript Code".to_string()),
        description: "Execute TypeScript code in a sandboxed environment using ts-node".to_string(),
        input_schema: object_schema(props, vec!["code".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_execute_shell_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "command".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(1000),
            Some("Shell command to execute"),
            vec![json!("ls -la"), json!("grep -r 'pattern' .")],
        ),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(1),
            Some(MAX_EXECUTION_TIMEOUT as i64),
            DEFAULT_EXECUTION_TIMEOUT as i64,
            Some("Timeout in seconds (default: 30)"),
        ),
    );
    props.insert(
        "working_dir".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("Working directory for command execution (optional)"),
        ),
    );

    MCPTool {
        name: "execute_shell".to_string(),
        title: Some("Execute Shell Command".to_string()),
        description: "Execute a shell command in the current environment".to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
