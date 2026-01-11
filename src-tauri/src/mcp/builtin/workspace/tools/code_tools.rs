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
            Some("Timeout in seconds (default: 30)"),
        ),
    );

    MCPTool {
        name: "runShell".to_string(),
        title: Some("Run Shell Command (Isolated)".to_string()),
        description: "Execute a shell command in an ISOLATED bash/sh session.\n\n\
                      ⚠️ PRIMARY TOOL: Use this for most shell commands (90% of cases).\n\n\
                      ISOLATION & STATE:\n\
                      - Medium isolation with restricted environment\n\
                      - NO state preservation - each call is independent\n\
                      - Synchronous execution with configurable timeout\n\n\
                      🔍 WORKING DIRECTORY:\n\
                      - Commands ALWAYS start from workspace root (project directory)\n\
                      - Use 'cd dir && command' to work in subdirectories\n\
                      - Example: 'cd src && ls' lists files in src/ directory\n\n\
                      USE CASES:\n\
                      - File operations: ls, cat, grep, find\n\
                      - Quick scripts: python script.py, node test.js\n\
                      - System info: pwd, whoami, env\n\
                      - Text processing: awk, sed, cut\n\n\
                      WHEN TO USE OTHER TOOLS:\n\
                      - Need persistent state (cd, export)? → Use runInPersistentShell\n\
                      - Long-running task (>30s)? → Use spawnProcess\n\n\
                      PLATFORM: Unix (Linux, macOS) - uses bash or sh shell."
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
            Some("Timeout in seconds (default: 30)"),
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
        description: "Execute a shell command using a PERSISTENT bash/sh session.\n\n\
                      ⚠️ ADVANCED TOOL: Only use when you need state preservation.\n\
                      For most commands (ls, cat, grep), use runShell instead.\n\n\
                      STATE PRESERVATION:\n\
                      - Variables (export VAR=value) persist between calls\n\
                      - Working directory (cd) persists between calls\n\
                      - Shell history and environment are maintained\n\
                      - NOT fully sandboxed - inherits host environment\n\n\
                      🔍 WORKING DIRECTORY BEHAVIOR:\n\
                      - Persistent shell tracks its own CWD (use 'pwd' to check)\n\
                      - 'cd' commands change the shell's CWD for future commands\n\
                      - ⚠️ FILE TOOLS IGNORE THIS: readFile/listDirectory always use workspace root\n\
                      - To list files in shell's CWD, use shell commands: 'ls' or 'find'\n\n\
                      INTERACTIVE INPUT:\n\
                      - Set 'requireUserInput: true' for sudo/interactive commands\n\
                      - Auto-detects privilege escalation (sudo, su, doas, pkexec)\n\
                      - ⚠️ LIMITATION: Only ONE input prompt supported\n\n\
                      USE CASES:\n\
                      - Navigating directories: cd, pushd, popd\n\
                      - Setting up environment: source, export\n\
                      - Running quick commands: ls, cat, grep\n\
                      - For long-running tasks (>30s), use spawnProcess\n\n\
                      PLATFORM: Unix (Linux, macOS) - uses bash or sh shell."
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

    MCPTool {
        name: "spawnProcess".to_string(),
        title: Some("Spawn Background Process".to_string()),
        description: "Spawn a shell command as a BACKGROUND PROCESS.\n\n\
                      ⚠️ ISOLATION & STATE:\n\
                      - NO state preservation - each call is independent\n\
                      - Medium isolation with restricted environment\n\
                      - Returns process_id immediately (non-blocking)\n\n\
                      🔍 WORKING DIRECTORY:\n\
                      - ⚠️ Commands ALWAYS execute from workspace root (the project directory)\n\
                      - Your command runs as if typed in a terminal opened at workspace root\n\
                      - To run in subdirectories, prefix with 'cd': \"cd src && npm install\"\n\
                      - No persistent directory - each call starts fresh at workspace root\n\
                      - Example: \"cd src && ls\" lists files in src/ directory\n\n\
                      PROCESS MANAGEMENT:\n\
                      - Use pollProcess(process_id) to check status\n\
                      - Use readProcessOutput(process_id) to get output\n\
                      - Use stopProcess(process_id) to cancel\n\
                      - Use listProcesses() to see all running processes\n\n\
                      USE CASES:\n\
                      - Long-running builds (npm run build, make)\n\
                      - Training models or heavy computations\n\
                      - File downloads or processing\n\
                      - Any command expected to take >30 seconds\n\n\
                      ⚠️ NO INTERACTIVE INPUT:\n\
                      - Background processes cannot prompt for input\n\
                      - For sudo commands, use runInPersistentShell with requireUserInput\n\n\
                      PLATFORM: Unix (Linux, macOS) - uses bash or sh shell."
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
            Some("Timeout in seconds (default: 30)"),
        ),
    );

    MCPTool {
        name: "runPowerShell".to_string(),
        title: Some("Run PowerShell Command (Isolated)".to_string()),
        description: "Execute a PowerShell command in an ISOLATED PowerShell session.\n\n\
                      ⚠️ PRIMARY TOOL: Use this for most PowerShell commands (90% of cases).\n\n\
                      ISOLATION & STATE:\n\
                      - Medium isolation with restricted environment\n\
                      - NO state preservation - each call is independent\n\
                      - Synchronous execution with configurable timeout\n\n\
                      🔍 WORKING DIRECTORY:\n\
                      - Commands ALWAYS start from workspace root (project directory)\n\
                      - Use 'Set-Location dir; command' to work in subdirectories\n\
                      - Example: 'Set-Location src; Get-ChildItem' lists src/ files\n\n\
                      USE CASES:\n\
                      - File operations: Get-ChildItem, Get-Content, Select-String\n\
                      - Quick scripts: python script.py, node test.js\n\
                      - System info: Get-Location, whoami, Get-Process\n\
                      - Text processing: Select-String, ForEach-Object\n\n\
                      WHEN TO USE OTHER TOOLS:\n\
                      - Need persistent state (Set-Location, $env:)? → Use runInPersistentPowerShell\n\
                      - Long-running task (>30s)? → Use spawnProcess\n\n\
                      PLATFORM: Windows - uses PowerShell."
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
            Some("Timeout in seconds (default: 30)"),
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
        description: "Execute a command using a PERSISTENT PowerShell session.\n\n\
                      ⚠️ ADVANCED TOOL: Only use when you need state preservation.\n\
                      For most commands (Get-ChildItem, Get-Content), use runPowerShell instead.\n\n\
                      STATE PRESERVATION:\n\
                      - Variables ($VAR=value) persist between calls\n\
                      - Working directory (Set-Location) persists between calls\n\
                      - PowerShell environment is maintained\n\
                      - NOT fully sandboxed - inherits host environment\n\n\
                      🔍 WORKING DIRECTORY BEHAVIOR:\n\
                      - Persistent shell tracks its own CWD (use Get-Location)\n\
                      - Set-Location (cd) changes CWD for future commands\n\
                      - ⚠️ FILE TOOLS IGNORE THIS: readFile/listDirectory use workspace root\n\
                      - To list files in shell's CWD, use: Get-ChildItem\n\n\
                      INTERACTIVE INPUT:\n\
                      - Set 'requireUserInput: true' for interactive commands\n\
                      - No auto-detection on Windows (must be explicit)\n\
                      - ⚠️ LIMITATION: Only ONE input prompt supported\n\n\
                      WINDOWS TIPS:\n\
                      - Use double quotes for paths with spaces\n\
                      - 'python' not found? Try 'py' or 'where.exe python'\n\
                      - 'where' failed? Use 'where.exe' (PowerShell alias conflict)\n\n\
                      USE CASES:\n\
                      - Navigating directories: Set-Location, Push-Location\n\
                      - Setting variables: $env:VAR=value\n\
                      - Quick commands: Get-ChildItem, Get-Content\n\
                      - For long tasks (>30s), use spawnProcess"
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
                json!("cd ./project; make all"),
            ],
        ),
    );

    MCPTool {
        name: "spawnProcess".to_string(),
        title: Some("Spawn Background Process".to_string()),
        description: "Spawn a shell command as a BACKGROUND PROCESS.\n\n\
                      ⚠️ ISOLATION & STATE:\n\
                      - NO state preservation - each call is independent\n\
                      - Medium isolation with restricted environment\n\
                      - Returns process_id immediately (non-blocking)\n\n\
                      🔍 WORKING DIRECTORY:\n\
                      - ⚠️ Commands ALWAYS execute from workspace root (the project directory)\n\
                      - Your command runs as if typed in PowerShell opened at workspace root\n\
                      - To run in subdirectories, prefix with 'Set-Location': \"Set-Location src; npm install\"\n\
                      - No persistent directory - each call starts fresh at workspace root\n\
                      - Example: \"Set-Location src; Get-ChildItem\" lists files in src/ directory\n\n\
                      PROCESS MANAGEMENT:\n\
                      - Use pollProcess(process_id) to check status\n\
                      - Use readProcessOutput(process_id) to get output\n\
                      - Use stopProcess(process_id) to cancel\n\
                      - Use listProcesses() to see all running processes\n\n\
                      USE CASES:\n\
                      - Long-running builds (npm, msbuild)\n\
                      - Training models or heavy computations\n\
                      - File processing or downloads\n\
                      - Any command expected to take >30 seconds\n\n\
                      ⚠️ NO INTERACTIVE INPUT:\n\
                      - Background processes cannot prompt for input\n\
                      - For interactive commands, use runInPersistentPowerShell\n\n\
                      PLATFORM: Windows - uses PowerShell (powershell.exe)"
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
    description: "Execute a pending shell command with user input.\n\n\
                  This tool is called automatically by the UIResource after user input.\n\
                  DO NOT call this tool directly - it is triggered by user interaction.\n\n\
                  FLOW:\n\
                  1. Agent calls runInPersistentShell with requireUserInput: true\n\
                  2. Agent receives UIResource with executionId\n\
                  3. User enters input in UIResource\n\
                  4. UIResource calls this tool with executionId and userInput\n\
                  5. Agent receives final stdout/stderr result\n\n\
                  SECURITY:\n\
                  - userInput is passed through MCP but NOT logged in agent context\n\
                  - Commands are sanitized before logging (-S flags removed)\n\
                  - Passwords are cleared from memory immediately after use";
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
    description: "Cancel a pending shell execution without executing it.\n\n\
                  This tool is called automatically when user clicks Cancel in UIResource.\n\
                  DO NOT call this tool directly - it is triggered by user interaction.\n\n\
                  FLOW:\n\
                  1. Agent calls runInPersistentShell with requireUserInput: true\n\
                  2. Agent receives UIResource with executionId\n\
                  3. User clicks Cancel button in UIResource\n\
                  4. UIResource calls this tool with executionId\n\
                  5. Pending execution is removed from state";
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
