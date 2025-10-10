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
            Some("Shell command to execute"),
            vec![
                json!("ls -la"),
                json!("grep -r 'pattern' ."),
                json!("source script.sh"),
            ],
        ),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(1),
            Some(crate::config::max_execution_timeout() as i64),
            crate::config::default_execution_timeout() as i64,
            Some("Timeout in seconds (sync mode only, default: 30)"),
        ),
    );
    props.insert(
        "run_mode".to_string(),
        enum_prop(
            vec!["sync", "async"],
            "sync",
            Some("Execution mode: 'sync' (wait for completion), 'async' (return immediately with process_id)"),
        ),
    );
    props.insert(
        "isolation".to_string(),
        enum_prop(
            vec!["basic", "medium", "high"],
            "medium",
            Some("Isolation level: 'basic' (env only), 'medium' (process groups), 'high' (sandboxing)"),
        ),
    );
    // 'working_dir' intentionally removed from the public tool schema to
    // prevent agents from changing execution directories. The server will
    // always execute commands within the session workspace path.

    MCPTool {
        name: "execute_shell".to_string(),
        title: Some("Execute Shell Command".to_string()),
        description: "Execute a shell command in a sandboxed environment.\n\n\
                      MODES:\n\
                      - 'sync' (default): Wait for completion, return stdout/stderr immediately\n\
                      - 'async': Run in background, return process_id immediately\n\n\
                      For async mode, use 'poll_process' to check status and retrieve output."
            .to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
