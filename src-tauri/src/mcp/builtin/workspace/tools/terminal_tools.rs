use crate::mcp::{utils::schema_builder::*, MCPTool};
use std::collections::HashMap;

/// Create poll_process tool
pub fn create_poll_process_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "processId".to_string(),
        string_prop_required("Process ID returned by spawnProcess"),
    );

    // Optional tail parameter
    let tail_props = vec![
        (
            "src".to_string(),
            enum_prop(
                vec!["stdout", "stderr"],
                "stdout",
                Some("Stream to read from"),
            ),
        ),
        (
            "n".to_string(),
            integer_prop_with_default(
                Some(1),
                Some(100),
                10,
                Some("Number of lines to tail (max 100)"),
            ),
        ),
    ];

    props.insert(
        "tail".to_string(),
        object_prop(
            tail_props,
            Vec::new(),
            Some("Get last N lines from stdout or stderr"),
        ),
    );

    MCPTool {
        name: "pollProcess".to_string(),
        title: Some("Poll Process Status".to_string()),
        description: "Check the status of an asynchronously running process. \
                      Optionally retrieve the last N lines of output (max 100 lines). \
                      Only processes from the current session can be queried."
            .to_string(),
        input_schema: object_schema(props, vec!["processId".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Create read_process_output tool
pub fn create_read_process_output_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert("processId".to_string(), string_prop_required("Process ID"));

    props.insert(
        "stream".to_string(),
        enum_prop_required(vec!["stdout", "stderr"], "Stream to read from"),
    );

    props.insert(
        "mode".to_string(),
        enum_prop(
            vec!["tail", "head"],
            "tail",
            Some("Read mode: 'tail' for last N lines, 'head' for first N lines"),
        ),
    );

    props.insert(
        "lines".to_string(),
        integer_prop_with_default(
            Some(1),
            Some(100),
            20,
            Some("Number of lines to read (max 100)"),
        ),
    );

    MCPTool {
        name: "readProcessOutput".to_string(),
        title: Some("Read Process Output".to_string()),
        description: "Read stdout or stderr from a background process. \
                      TEXT OUTPUT ONLY. Maximum 100 lines per request. \
                      Use 'tail' mode for last N lines, 'head' for first N lines."
            .to_string(),
        input_schema: object_schema(props, vec!["processId".to_string(), "stream".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Create list_processes tool
pub fn create_list_processes_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "statusFilter".to_string(),
        enum_prop(
            vec!["all", "running", "finished"],
            "all",
            Some("Filter by status: 'all' (default), 'running', or 'finished'"),
        ),
    );

    MCPTool {
        name: "listProcesses".to_string(),
        title: Some("List Processes".to_string()),
        description: "List all background processes in the current session. \
                      Filter by status: 'all' (default), 'running', or 'finished' (includes failed)."
            .to_string(),
        input_schema: object_schema(props, vec![]),
        output_schema: None,
        annotations: None,
    }
}

/// Create stop_process tool
pub fn create_stop_process_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "processId".to_string(),
        string_prop_required("Process ID to stop"),
    );

    MCPTool {
        name: "stopProcess".to_string(),
        title: Some("Stop Process".to_string()),
        description: "Stop a running background process. \
                      Sends a termination signal to the process."
            .to_string(),
        input_schema: object_schema(props, vec!["processId".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Create createInteractiveShell tool
pub fn create_interactive_shell_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "shellId".to_string(),
        string_prop_optional("Custom shell identifier (defaults to session ID)"),
    );

    MCPTool {
        name: "createInteractiveShell".to_string(),
        title: Some("Create Interactive Shell".to_string()),
        description: "Create a new interactive shell session or reuse existing one. \
                      Returns shell metadata (CWD, PID, status). Shell persists across tool calls. \
                      Uses PTY for true interactive behavior (REPLs, TUI)."
            .to_string(),
        input_schema: object_schema(props, vec![]),
        output_schema: None,
        annotations: None,
    }
}

/// Create writeToInteractiveShell tool
pub fn create_write_interactive_shell_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "shellId".to_string(),
        string_prop_optional("Shell identifier (defaults to session ID)"),
    );

    props.insert(
        "input".to_string(),
        string_prop_required("Input text to send to shell stdin"),
    );

    props.insert(
        "sendNewline".to_string(),
        boolean_prop_with_default(
            true,
            Some("Append newline after input (default: true)"),
        ),
    );

    MCPTool {
        name: "writeToInteractiveShell".to_string(),
        title: Some("Write to Interactive Shell".to_string()),
        description: "Write input to shell stdin without waiting for output. \
                      Use for interactive commands that require step-by-step input. \
                      Follow with readFromInteractiveShell to get response."
            .to_string(),
        input_schema: object_schema(props, vec!["input".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Create readFromInteractiveShell tool
pub fn create_read_interactive_shell_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "shellId".to_string(),
        string_prop_optional("Shell identifier (defaults to session ID)"),
    );

    props.insert(
        "timeoutMs".to_string(),
        integer_prop_with_default(
            Some(100),
            Some(10000),
            1000,
            Some("Timeout in milliseconds (default: 1000)"),
        ),
    );

    props.insert(
        "waitForPattern".to_string(),
        string_prop_optional("Regex pattern to wait for in output (e.g., ':', '>', '[Y/n]')"),
    );

    MCPTool {
        name: "readFromInteractiveShell".to_string(),
        title: Some("Read from Interactive Shell".to_string()),
        description: "Read available output from shell stdout/stderr. \
                      Non-blocking: returns after timeout or when pattern found. \
                      Use waitForPattern to detect interactive prompts. \
                      Note: PTY merges stdout and stderr."
            .to_string(),
        input_schema: object_schema(props, vec![]),
        output_schema: None,
        annotations: None,
    }
}

/// Create killInteractiveShell tool
pub fn create_kill_interactive_shell_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "shellId".to_string(),
        string_prop_optional("Shell identifier to terminate (defaults to session ID)"),
    );

    MCPTool {
        name: "killInteractiveShell".to_string(),
        title: Some("Kill Interactive Shell".to_string()),
        description: "Terminate an interactive shell session and clean up resources. \
                      Shell state (CWD, env vars) is lost after termination."
            .to_string(),
        input_schema: object_schema(props, vec![]),
        output_schema: None,
        annotations: None,
    }
}
