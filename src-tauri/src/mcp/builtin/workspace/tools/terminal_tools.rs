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

// --- New Interactive Shell Tools ---

/// Create create_interactive_shell tool
pub fn create_create_interactive_shell_tool() -> MCPTool {
    let mut props = HashMap::new();
    // No params needed, uses session_id from context, but we allow optional size in future
    // For now, simple.

    MCPTool {
        name: "createInteractiveShell".to_string(),
        title: Some("Create Interactive Shell".to_string()),
        description: "Create a new interactive shell session (PTY) for the current user session. \
                      If a shell already exists, it does nothing."
            .to_string(),
        input_schema: object_schema(props, vec![]),
        output_schema: None,
        annotations: None,
    }
}

/// Create write_to_interactive_shell tool
pub fn create_write_to_interactive_shell_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "data".to_string(),
        string_prop_required("Data to write to the shell (e.g., command + newline)"),
    );

    MCPTool {
        name: "writeToInteractiveShell".to_string(),
        title: Some("Write to Interactive Shell".to_string()),
        description: "Write data (input) to the interactive shell PTY. \
                      Use this to send commands or interact with prompts. \
                      Automatically creates a shell if one does not exist."
            .to_string(),
        input_schema: object_schema(props, vec!["data".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Create read_from_interactive_shell tool
pub fn create_read_from_interactive_shell_tool() -> MCPTool {
    let mut props = HashMap::new();

    MCPTool {
        name: "readFromInteractiveShell".to_string(),
        title: Some("Read from Interactive Shell".to_string()),
        description: "Read pending output from the interactive shell PTY. \
                      Returns any data buffered since the last read. \
                      Output contains both stdout and stderr merged."
            .to_string(),
        input_schema: object_schema(props, vec![]),
        output_schema: None,
        annotations: None,
    }
}

/// Create kill_interactive_shell tool
pub fn create_kill_interactive_shell_tool() -> MCPTool {
    let mut props = HashMap::new();

    MCPTool {
        name: "killInteractiveShell".to_string(),
        title: Some("Kill Interactive Shell".to_string()),
        description: "Terminate the current interactive shell session."
            .to_string(),
        input_schema: object_schema(props, vec![]),
        output_schema: None,
        annotations: None,
    }
}
