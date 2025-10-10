use crate::mcp::{utils::schema_builder::*, MCPTool};
use serde_json::json;
use std::collections::HashMap;

pub fn create_execute_shell_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "command".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(1000),
            Some("Shell command to execute (POSIX sh compatible)"),
            vec![
                json!("ls -la"),
                json!("grep -r 'pattern' ."),
                json!(". script.sh"),
            ],
        ),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(1),
            Some(crate::config::max_execution_timeout() as i64),
            crate::config::default_execution_timeout() as i64,
            Some("Timeout in seconds (default: 30)"),
        ),
    );
    // 'working_dir' intentionally removed from the public tool schema to
    // prevent agents from changing execution directories. The server will
    // always execute commands within the session workspace path.

    MCPTool {
        name: "execute_shell".to_string(),
        title: Some("Execute Shell Command".to_string()),
        description: "Execute a shell command in a sandboxed environment using POSIX sh shell. Note: bash-specific commands like 'source' are not available - use '.' instead for sourcing files. Only basic POSIX shell features are supported.".to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
