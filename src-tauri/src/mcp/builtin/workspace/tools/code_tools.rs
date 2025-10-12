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
    // Isolation level removed - always use Medium isolation for security
    // This prevents AI agents from choosing weaker isolation that could be exploited
    // 'working_dir' intentionally removed from the public tool schema to
    // prevent agents from changing execution directories. The server will
    // always execute commands within the session workspace path.

    MCPTool {
        name: "execute_shell".to_string(),
        title: Some("Execute Shell Command (bash/sh)".to_string()),
        description: "Execute a shell command using bash or sh in a sandboxed environment.\n\n\
                      MODES:\n\
                      - 'sync' (default): Wait for completion, return stdout/stderr immediately\n\
                      - 'async': Run in background, return process_id immediately\n\n\
                      For async mode, use 'poll_process' to check status and retrieve output.\n\n\
                      PLATFORM: Unix (Linux, macOS) - uses bash or sh shell."
            .to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

// Windows platform tool (cmd.exe)
#[cfg(windows)]
pub fn create_execute_shell_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "command".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(1000),
            Some("Command to execute using cmd.exe"),
            vec![
                json!("dir /b"),
                json!("echo Hello World"),
                json!("type file.txt"),
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
    // Isolation level removed - always use Medium isolation for security
    // This prevents AI agents from choosing weaker isolation that could be exploited
    // 'working_dir' intentionally removed from the public tool schema to
    // prevent agents from changing execution directories. The server will
    // always execute commands within the session workspace path.

    MCPTool {
        name: "execute_windows_cmd".to_string(),
        title: Some("Execute Windows Command (cmd.exe)".to_string()),
        description: "Execute a command using Windows cmd.exe in a sandboxed environment.\n\n\
                      MODES:\n\
                      - 'sync' (default): Wait for completion, return stdout/stderr immediately\n\
                      - 'async': Run in background, return process_id immediately\n\n\
                      For async mode, use 'poll_process' to check status and retrieve output.\n\n\
                      PLATFORM: Windows - uses cmd.exe (Command Prompt).\n\
                      IMPORTANT NOTES:\n\
                      - Commands are executed via 'cmd /S /C' for proper quote handling\n\
                      - Use double quotes for paths with spaces: dir \"C:\\Program Files\"\n\
                      - To call external programs (ffmpeg, python, etc.), ensure they are in PATH\n\
                      - For UTF-8 filenames, the system handles encoding automatically\n\
                      - Simple commands work best; for complex scripts consider writing to a .bat file first"
            .to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
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
                assert!(examples.iter().any(|e| e.as_str().unwrap().contains("dir")));
                assert!(examples
                    .iter()
                    .any(|e| e.as_str().unwrap().contains("echo")));
            }
            _ => panic!("Expected Object schema type"),
        }
    }
}
