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
            Some("Read mode: 'tail' reads last N lines, 'head' reads first N lines"),
        ),
    );

    props.insert(
        "lines".to_string(),
        integer_prop_with_default(Some(1), Some(100), 20, Some("Number of lines to read")),
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
        description: "Read captured stdout or stderr from a background process.".to_string(),
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
        string_prop_required("Process ID to wait for"),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(0),
            Some(3600),
            30,
            Some(
                "Timeout in seconds. Use 0 to return current status immediately without blocking.",
            ),
        ),
    );

    MCPTool {
        name: "waitForProcess".to_string(),
        title: Some("Wait For Process".to_string()),
        description: "Block until a background process finishes or times out. Returns process status and metadata."
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
            Some("Filter by status"),
        ),
    );

    MCPTool {
        name: "listProcesses".to_string(),
        title: Some("List Processes".to_string()),
        description: "List background processes in this session.".to_string(),
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
        description: "Terminate a running background process immediately.".to_string(),
        input_schema: object_schema(props, vec!["processId".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
