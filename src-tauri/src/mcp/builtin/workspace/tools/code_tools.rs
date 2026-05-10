use crate::mcp::{utils::schema_builder::*, MCPTool};
use serde_json::json;
use std::collections::HashMap;

// Unix platform tool (bash/sh) - PRIMARY TOOL (Isolated Shell)
#[cfg(unix)]
pub fn create_run_shell_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "command".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(1000),
            Some("Shell command to execute (bash/sh)"),
            vec![
                json!("ls -la"),
                json!("cat README.md"),
                json!("grep -r 'pattern' src/"),
            ],
        ),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(1),
            Some(crate::config::max_execution_timeout() as i64),
            crate::config::default_execution_timeout() as i64,
            Some("Timeout in seconds"),
        ),
    );

    MCPTool {
        name: "runShell".to_string(),
        title: Some("Run Shell Command (Isolated)".to_string()),
        description: "Run a synchronous shell command (bash/sh). Stateless — each call starts fresh at workspace root.\n\
                      Use 'cd dir && command' for subdirectories.\n\
                      For persistent cd/env vars: runInPersistentShell. For commands >30s: spawnProcess."
            .to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

// Unix platform tool (bash/sh) - ADVANCED TOOL (Persistent Shell)
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
                json!("cd src && ls"),
                json!("export VAR=value && echo $VAR"),
            ],
        ),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(1),
            Some(crate::config::max_execution_timeout() as i64),
            crate::config::default_execution_timeout() as i64,
            Some("Timeout in seconds"),
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
        "inputType".to_string(),
        enum_prop(
            vec!["password", "text"],
            "text",
            Some("Input type: 'password' (hidden) or 'text' (visible). Auto-set to 'password' for sudo commands."),
        ),
    );

    MCPTool {
        name: "runInPersistentShell".to_string(),
        title: Some("Execute Shell Command (Persistent Session)".to_string()),
        description: "Run a shell command in a persistent session that preserves working directory and env vars across calls.\n\
                       Use when you need 'cd' to stick, 'export' to carry forward, or interactive commands (sudo).\n\
                       File tools accept relative paths from the workspace or absolute paths, but they do not follow the shell's current directory automatically.\n\
                       For simple stateless commands: runShell."
            .to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

// Background process spawning tool (platform-agnostic)
#[cfg(unix)]
pub fn create_spawn_process_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "command".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(1000),
            Some("Shell command to execute as background process"),
            vec![
                json!("cd src && npm run build"),
                json!("python train_model.py --epochs 100"),
                json!("cd ./project && make all"),
            ],
        ),
    );

    props.insert(
        "name".to_string(),
        string_prop(
            None,
            Some(100),
            Some("Optional label shown in spawnProcess/listProcesses results. Control tools still require the returned processId."),
        ),
    );

    MCPTool {
        name: "spawnProcess".to_string(),
        title: Some("Spawn Background Process".to_string()),
        description: "Start a command as a non-blocking background process. Returns process_id immediately.\n\
                       Optional name is a label only; waitForProcess, stopProcess, and readProcessOutput still require process_id.\n\
                       Stateless — starts from workspace root each call. No interactive input.\n\
                       Use waitForProcess(id) to wait for completion, readProcessOutput(id) to get output."
            .to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

// Windows platform tool (PowerShell) - PRIMARY TOOL (Isolated PowerShell)
#[cfg(windows)]
pub fn create_run_powershell_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "command".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(1000),
            Some("PowerShell command to execute"),
            vec![
                json!("Get-ChildItem"),
                json!("Get-Content README.md"),
                json!("Get-Process | Select-Object -First 10"),
            ],
        ),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(1),
            Some(crate::config::max_execution_timeout() as i64),
            crate::config::default_execution_timeout() as i64,
            Some("Timeout in seconds"),
        ),
    );

    MCPTool {
        name: "runPowerShell".to_string(),
        title: Some("Run PowerShell Command (Isolated)".to_string()),
        description: "Execute a synchronous PowerShell command on Windows. This is the primary tool for Windows command-line tasks.

Guidelines:
- Use ';' to chain multiple commands (e.g. 'cd src; pnpm test'). Note: '&&' is not supported in PowerShell 5.1.
- Access environment variables using '$env:VARNAME'.
- Each call starts fresh at the workspace root. For persistent state, use runInPersistentPowerShell."
            .to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

// Windows platform tool (PowerShell) - ADVANCED TOOL (Persistent Shell)
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
            Some("Timeout in seconds"),
        ),
    );
    props.insert("requireUserInput".to_string(), {
        let mut schema = boolean_prop(Some(
            "Request user input before execution (e.g., for interactive prompts).",
        ));
        schema.default = Some(json!(false));
        schema
    });
    props.insert(
        "inputPrompt".to_string(),
        string_prop(None, None, Some("Custom prompt message for user input")),
    );
    props.insert(
        "inputType".to_string(),
        enum_prop(
            vec!["password", "text"],
            "text",
            Some("Input type: 'password' (hidden) or 'text' (visible)"),
        ),
    );

    MCPTool {
        name: "runInPersistentPowerShell".to_string(),
        title: Some("Execute PowerShell Command (Persistent Session)".to_string()),
        description: "Run PowerShell in a persistent session that preserves location (Set-Location) and env vars across calls.\n\
                       - Use ';' to chain commands.\n\
                       - File tools accept relative paths from the workspace or absolute paths, but they do not follow the shell's current location automatically.\n\
                       - For simple stateless commands: runPowerShell."
            .to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

// Background process spawning tool (platform-agnostic)
#[cfg(windows)]
pub fn create_spawn_process_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "command".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(1000),
            Some("PowerShell command to execute as background process"),
            vec![
                json!("Set-Location src; npm run build"),
                json!("python train_model.py --epochs 100"),
                json!("Set-Location ./project; make all"),
            ],
        ),
    );

    props.insert(
        "name".to_string(),
        string_prop(
            None,
            Some(100),
            Some("Optional label shown in spawnProcess/listProcesses results. Control tools still require the returned processId."),
        ),
    );

    MCPTool {
        name: "spawnProcess".to_string(),
        title: Some("Spawn Background Process".to_string()),
        description: "Start a command as a non-blocking background process. Returns process_id immediately.\n\
                       Optional name is a label only; waitForProcess, stopProcess, and readProcessOutput still require process_id.\n\
                       Stateless — starts from workspace root each call. No interactive input.\n\
                       Use waitForProcess(id) to wait for completion, readProcessOutput(id) to get output."
            .to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

// Macro to unify tool constant definition and creation function
macro_rules! define_mcp_tool {
    (
        const $const_name:ident = $tool_name:expr;
        fn $fn_name:ident();
        title: $title:expr;
        description: $desc:expr;
        inputs: $props_ident:ident => $props_block:block;
        required: $required:expr;
    ) => {
        pub const $const_name: &str = $tool_name;

        pub fn $fn_name() -> MCPTool {
            let mut $props_ident = HashMap::new();
            $props_block

            MCPTool {
                name: $const_name.to_string(),
                title: Some($title.to_string()),
                description: $desc.to_string(),
                input_schema: object_schema($props_ident, $required),
                output_schema: None,
                annotations: None,
            }
        }
    };
}

define_mcp_tool! {
    const EXECUTE_PENDING_SHELL = "executePendingShell";
    fn create_execute_pending_shell_tool();
    title: "Execute Pending Shell Command";
    description: "[Internal callback] Continue a pending interactive shell execution with executionId and userInput.";
    inputs: props => {
        props.insert(
            "executionId".to_string(),
            string_prop(
                None,
                None,
                Some("Execution ID returned from runInPersistentShell with requireUserInput"),
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
    };
    required: vec!["executionId".to_string(), "userInput".to_string()];
}

define_mcp_tool! {
    const CANCEL_PENDING_EXECUTION = "cancelPendingExecution";
    fn create_cancel_pending_execution_tool();
    title: "Cancel Pending Execution";
    description: "[Internal callback] Cancel a pending interactive shell execution by executionId.";
    inputs: props => {
        props.insert(
            "executionId".to_string(),
            string_prop(
                None,
                None,
                Some("Execution ID of the pending shell command to cancel"),
            ),
        );
    };
    required: vec!["executionId".to_string()];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_name_platform_specific() {
        let tool = create_execute_shell_tool();

        #[cfg(unix)]
        assert_eq!(tool.name, "runInPersistentShell");

        #[cfg(windows)]
        assert_eq!(tool.name, "runInPersistentPowerShell");
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
                assert!(props.contains_key("requireUserInput"));
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

                // Unix command examples verification
                assert!(examples.iter().any(|e| e.as_str().unwrap().contains("ls")));
                assert!(examples.iter().any(|e| e.as_str().unwrap().contains("cd")));
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
