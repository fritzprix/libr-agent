use crate::mcp::{utils::schema_builder::*, MCPTool};
use serde_json::json;
use std::collections::HashMap;

use super::super::utils::constants::{
    DEFAULT_EXECUTION_TIMEOUT, MAX_CODE_SIZE, MAX_EXECUTION_TIMEOUT,
};

// Note: Python and TypeScript execution tools were intentionally removed from
// the public MCP tool schema. They required external runtime dependencies
// and allowed agents to influence isolation/permissions. The server still
// contains internal handlers for execution in certain contexts, but these
// are not exposed as MCP tools to prevent misuse.

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
    // 'working_dir' intentionally removed from the public tool schema to
    // prevent agents from changing execution directories. The server will
    // always execute commands within the session workspace path.

    MCPTool {
        name: "execute_shell".to_string(),
        title: Some("Execute Shell Command".to_string()),
        description: "Execute a shell command in a sandboxed environment".to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_eval_javascript_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "code".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(MAX_CODE_SIZE as u32),
            Some("JavaScript code to evaluate"),
            vec![json!("console.log('Hello, World!');"), json!("2 + 2")],
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
        name: "eval_javascript".to_string(),
        title: Some("Evaluate JavaScript Code".to_string()),
        description: "Evaluate JavaScript code using the Boa JavaScript engine".to_string(),
        input_schema: object_schema(props, vec!["code".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
