use crate::mcp::{utils::schema_builder::*, MCPTool};
use std::collections::HashMap;

/// Create read_process_output tool
pub fn create_read_process_output_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "processId".to_string(),
        string_prop_required("Process ID returned by spawnProcess or listProcesses"),
    );

    props.insert(
        "stream".to_string(),
        enum_prop_required(
            vec!["stdout", "stderr", "both"],
            "Stream to read: stdout, stderr, or both in one call",
        ),
    );

    props.insert(
        "mode".to_string(),
        enum_prop(
            vec!["tail", "head"],
            "tail",
            Some("Read mode: 'tail' reads the latest lines, 'head' reads the earliest lines"),
        ),
    );

    props.insert(
        "lines".to_string(),
        integer_prop_with_default(Some(1), Some(100), 20, Some("Number of lines to read")),
    );

    MCPTool {
        name: "readProcessOutput".to_string(),
        title: Some("Read Process Output".to_string()),
        description:
            "Read captured stdout, stderr, or both streams from a background process using head/tail line windows. Returns output_paths so file tools can inspect the full captured files."
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
        string_prop_required("Process ID returned by spawnProcess or listProcesses"),
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
        string_prop_required("Process ID returned by spawnProcess or listProcesses"),
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
