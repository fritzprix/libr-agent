use crate::mcp::{utils::schema_builder::*, MCPTool};
use std::collections::HashMap;

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

    props.insert(
        "start_line".to_string(),
        integer_prop(
            Some(1),
            None,
            Some("Start line number (1-based, inclusive)"),
        ),
    );
    props.insert(
        "end_line".to_string(),
        integer_prop(Some(1), None, Some("End line number (1-based, inclusive)")),
    );

    MCPTool {
        name: "readProcessOutput".to_string(),
        title: Some("Read Process Output".to_string()),
        description: "Read stdout or stderr from a background process.\n\n\
                      BEHAVIOR:\n\
                      - TEXT OUTPUT ONLY. Max 100 lines per request.\n\
                      - Supports 'tail' (end of file) or 'head' (beginning of file) modes.\n\
                      - Supports specific ranges via 'start_line' and 'end_line' (1-based).\n\n\
                      WORKFLOW:\n\
                      1. Spawn a process using spawnProcess.\n\
                      2. Wait for it to finish or check progress using waitForProcess.\n\
                      3. Read output using this tool to see what happened.\n\n\
                      USAGE:\n\
                      - Use 'stdout' for successful outputs or progress logs.\n\
                      - Use 'stderr' to check for error messages if the process failed."
            .to_string(),
        input_schema: object_schema(props, vec!["processId".to_string(), "stream".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Create wait_for_process tool
pub fn create_wait_for_process_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "processId".to_string(),
        string_prop_required("Process ID to wait for (or poll)"),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(0),
            Some(3600),
            30,
            Some("Timeout in seconds. Use 0 for immediate status check (polling)."),
        ),
    );

    MCPTool {
        name: "waitForProcess".to_string(),
        title: Some("Wait For / Poll Process".to_string()),
        description: "Wait for a background process to complete, or check its status.\n\n\
                      MODES:\n\
                      - timeout > 0 (Default 30s): BLOCKS until process finishes or timeout occurs.\n\
                      - timeout = 0: POLLS status immediately without blocking. Returns 'running', 'finished', 'failed', or 'killed'.\n\n\
                      WORKFLOW:\n\
                      - Use frequently after spawnProcess to determine when logs are ready to read.\n\
                      - If the process is still 'running' and you need its output, use readProcessOutput.\n\n\
                      RETURNS:\n\
                      - Full process metadata (pid, status, exit_code, timestamps, output sizes)."
            .to_string(),
        input_schema: object_schema(props, vec!["processId".to_string()]),
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
        description: "List all background processes in the current session.\n\n\
                      FILTERING:\n\
                      - 'all' (default): See everything started in this session.\n\
                      - 'running': Find processes currently consuming resources.\n\
                      - 'finished': See historical results (includes 'failed' and 'killed').\n\n\
                      USAGE:\n\
                      - Use this if you lose track of the process_id returned by spawnProcess.\n\
                      - Useful for getting an overview of concurrent activity in the workspace."
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
        description: "Stop a running background process immediately.\n\n\
                      BEHAVIOR:\n\
                      - Sends a termination signal (SIGTERM on Unix, taskkill /F on Windows).\n\
                      - Resource cleanup is performed automatically after termination.\n\n\
                      USAGE:\n\
                      - Use when a process is taking too long (detected via waitForProcess timeout).\n\
                      - Use to cancel long-running builds or tests that are no longer needed."
            .to_string(),
        input_schema: object_schema(props, vec!["processId".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
