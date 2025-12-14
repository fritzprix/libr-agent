use crate::mcp::{utils::schema_builder::*, MCPTool};
use serde_json::json;
use std::collections::HashMap;

// Unix platform tool (bash/sh)
#[cfg(unix)]
pub fn create_execute_shell_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "command".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(1000),
            Some("Shell command to execute (bash/sh)"),
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
        "requireUserInput".to_string(),
        {
            let mut schema = boolean_prop(Some("Request user input before execution (e.g., sudo password). Auto-detects sudo/su/doas/pkexec on Unix."));
            schema.default = Some(json!(false));
            schema
        },
    );
    props.insert(
        "inputPrompt".to_string(),
        string_prop(
            None,
            None,
            Some("Custom prompt message for user input (default: auto-detected based on command)"),
        ),
    );
    props.insert(
        "input_type".to_string(),
        enum_prop(
            vec!["password", "text"],
            "text",
            Some("Input type: 'password' (hidden) or 'text' (visible). Auto-set to 'password' for sudo commands."),
        ),
    );
    // Isolation level removed - always use Medium isolation for security
    // This prevents AI agents from choosing weaker isolation that could be exploited
    // 'working_dir' intentionally removed from the public tool schema to
    // prevent agents from changing execution directories. The server will
    // always execute commands within the session workspace path.

    MCPTool {
        name: "execute_shell".to_string(),
        title: Some("Execute Shell Command (bash/sh)".to_string()),
        description: "Execute a shell command using bash or sh.\n\n\
                      INTERACTIVE INPUT:\n\
                      - Set 'requireUserInput: true' to prompt for user input before execution\n\
                      - Auto-detects privilege escalation commands (sudo, su, doas, pkexec)\n\
                      - Supports password (hidden) and text (visible) input types\n\
                      - ⚠️ LIMITATION: Only supports SINGLE pre-execution input (stdin closed after input)\n\
                      - Multiple prompts (e.g., password → y/n confirmation) are NOT supported\n\n\
                      MODES:\n\
                      - 'sync' (default): Uses a PERSISTENT shell session. State (variables, working dir) is preserved between calls.\n\
                        ⚠️ SECURITY NOTE: This mode is NOT fully sandboxed. It inherits the host environment and allows navigation outside the workspace.\n\
                      - 'async': Runs in a background process with Medium Isolation (restricted env). Returns process_id immediately.\n\n\
                      For async mode, use 'poll_process' to check status and retrieve output.\n\n\
                      PLATFORM: Unix (Linux, macOS) - uses bash or sh shell."
            .to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

// Windows platform tool (PowerShell)
#[cfg(windows)]
pub fn create_execute_shell_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "command".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(1000),
            Some("Command to execute using PowerShell"),
            vec![
                json!("Get-ChildItem"),
                json!("Write-Host 'Hello World'"),
                json!("Get-Content file.txt"),
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
        "requireUserInput".to_string(),
        {
            let mut schema = boolean_prop(Some("Request user input before execution. Must be explicitly set on Windows (no auto-detection)."));
            schema.default = Some(json!(false));
            schema
        },
    );
    props.insert(
        "inputPrompt".to_string(),
        string_prop(None, None, Some("Custom prompt message for user input")),
    );
    props.insert(
        "input_type".to_string(),
        enum_prop(
            vec!["password", "text"],
            "text",
            Some("Input type: 'password' (hidden) or 'text' (visible)"),
        ),
    );
    // Isolation level removed - always use Medium isolation for security
    // This prevents AI agents from choosing weaker isolation that could be exploited
    // 'working_dir' intentionally removed from the public tool schema to
    // prevent agents from changing execution directories. The server will
    // always execute commands within the session workspace path.

    MCPTool {
        name: "execute_windows_cmd".to_string(),
        title: Some("Execute Windows Command (PowerShell)".to_string()),
        description: "Execute a command using Windows PowerShell.\n\n\
                      FEATURES:\n\
                      - Interactive Input: Set 'requireUserInput: true'. Supports text/password (single prompt only).\n\
                      - Modes: 'sync' (wait for output) or 'async' (background).\n\n\
                      MODES:\n\
                      - 'sync' (default): Uses a PERSISTENT shell session. State (variables, working dir) is preserved between calls.\n\
                        ⚠️ SECURITY NOTE: This mode is NOT fully sandboxed. It inherits the host environment/PATH and allows navigation outside the workspace.\n\
                      - 'async': Runs in a background process with Medium Isolation.\n\n\
                      WINDOWS TIPS:\n\
                      - Shell: PowerShell (powershell.exe).\n\
                      - Path: Use double quotes for paths with spaces.\n\
                      - External Tools: Ensure 'python', 'node', etc. are in PATH.\n\
                      - Troubleshooting:\n\
                        - 'python' not found? Try 'py' or 'where.exe python'.\n\
                        - 'where' command failed? Use 'where.exe' (PowerShell alias conflict).\n\
                        - 'del' failed? Use comma-separated paths: 'del file1, file2'."
            .to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Create execute_pending_shell tool (2nd tool in Two-Tool Pattern)
/// This tool is called automatically by UIResource after user input
pub fn create_execute_pending_shell_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "executionId".to_string(),
        string_prop(
            None,
            None,
            Some("Execution ID returned from execute_shell with requireUserInput"),
        ),
    );

    props.insert(
        "userInput".to_string(),
        string_prop(
            None,
            None,
            Some("User input (password or text) to inject into command stdin"),
        ),
    );

    MCPTool {
        name: "execute_pending_shell".to_string(),
        title: Some("Execute Pending Shell Command".to_string()),
        description: "Execute a pending shell command with user input.\n\n\
                      This tool is called automatically by the UIResource after user input.\n\
                      DO NOT call this tool directly - it is triggered by user interaction.\n\n\
                      FLOW:\n\
                      1. Agent calls execute_shell with requireUserInput: true\n\
                      2. Agent receives UIResource with executionId\n\
                      3. User enters input in UIResource\n\
                      4. UIResource calls this tool with executionId and userInput\n\
                      5. Agent receives final stdout/stderr result\n\n\
                      SECURITY:\n\
                      - userInput is passed through MCP but NOT logged in agent context\n\
                      - Commands are sanitized before logging (-S flags removed)\n\
                      - Passwords are cleared from memory immediately after use"
            .to_string(),
        input_schema: object_schema(
            props,
            vec!["executionId".to_string(), "userInput".to_string()],
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Create cancel_pending_execution tool
/// This tool is called from UIResource when user cancels the operation
pub fn create_cancel_pending_execution_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "executionId".to_string(),
        string_prop(
            None,
            None,
            Some("Execution ID of the pending shell command to cancel"),
        ),
    );

    MCPTool {
        name: "cancel_pending_execution".to_string(),
        title: Some("Cancel Pending Execution".to_string()),
        description: "Cancel a pending shell execution without executing it.\n\n\
                      This tool is called automatically when user clicks Cancel in UIResource.\n\
                      DO NOT call this tool directly - it is triggered by user interaction.\n\n\
                      FLOW:\n\
                      1. Agent calls execute_shell with requireUserInput: true\n\
                      2. Agent receives UIResource with executionId\n\
                      3. User clicks Cancel button in UIResource\n\
                      4. UIResource calls this tool with executionId\n\
                      5. Pending execution is removed from state"
            .to_string(),
        input_schema: object_schema(props, vec!["executionId".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_name_platform_specific() {
        let tool = create_execute_shell_tool();

        #[cfg(unix)]
        assert_eq!(tool.name, "execute_shell");

        #[cfg(windows)]
        assert_eq!(tool.name, "execute_windows_cmd");
    }

    #[test]
    fn test_tool_schema_has_required_properties() {
        use crate::mcp::schema::JSONSchemaType;

        let tool = create_execute_shell_tool();
        let schema = &tool.input_schema;

        // Check that input_schema is an Object type with properties
        match &schema.schema_type {
            JSONSchemaType::Object { properties, .. } => {
                assert!(properties.is_some());
                let props = properties.as_ref().unwrap();
                assert!(props.contains_key("command"));
                assert!(props.contains_key("timeout"));
                assert!(props.contains_key("run_mode"));
            }
            _ => panic!("Expected Object schema type"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_unix_tool_has_unix_examples() {
        use crate::mcp::schema::JSONSchemaType;

        let tool = create_execute_shell_tool();
        let schema = &tool.input_schema;

        // Get the command property and check its examples
        match &schema.schema_type {
            JSONSchemaType::Object { properties, .. } => {
                let props = properties.as_ref().unwrap();
                let command_schema = props.get("command").unwrap();
                let examples = command_schema.examples.as_ref().unwrap();

                // Unix 명령어 예제 확인
                assert!(examples.iter().any(|e| e.as_str().unwrap().contains("ls")));
                assert!(examples
                    .iter()
                    .any(|e| e.as_str().unwrap().contains("grep")));
            }
            _ => panic!("Expected Object schema type"),
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_tool_has_windows_examples() {
        use crate::mcp::schema::JSONSchemaType;

        let tool = create_execute_shell_tool();
        let schema = &tool.input_schema;

        // Get the command property and check its examples
        match &schema.schema_type {
            JSONSchemaType::Object { properties, .. } => {
                let props = properties.as_ref().unwrap();
                let command_schema = props.get("command").unwrap();
                let examples = command_schema.examples.as_ref().unwrap();

                // Windows 명령어 예제 확인
                assert!(examples
                    .iter()
                    .any(|e| e.as_str().unwrap().contains("Get-ChildItem")));
                assert!(examples
                    .iter()
                    .any(|e| e.as_str().unwrap().contains("Write-Host")));
            }
            _ => panic!("Expected Object schema type"),
        }
    }
}
