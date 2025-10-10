use crate::mcp::{utils::schema_builder::*, MCPTool};
use std::collections::HashMap;

/// Create poll_process tool
pub fn create_poll_process_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "process_id".to_string(),
        string_prop_required("Process ID returned by execute_shell (async mode)"),
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
        name: "poll_process".to_string(),
        title: Some("Poll Process Status".to_string()),
        description: "Check the status of an asynchronously running process. \
                      Optionally retrieve the last N lines of output (max 100 lines). \
                      Only processes from the current session can be queried."
            .to_string(),
        input_schema: object_schema(props, vec!["process_id".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Create read_process_output tool
pub fn create_read_process_output_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert("process_id".to_string(), string_prop_required("Process ID"));

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
        name: "read_process_output".to_string(),
        title: Some("Read Process Output".to_string()),
        description: "Read stdout or stderr from a background process. \
                      TEXT OUTPUT ONLY. Maximum 100 lines per request. \
                      Use 'tail' mode for last N lines, 'head' for first N lines."
            .to_string(),
        input_schema: object_schema(props, vec!["process_id".to_string(), "stream".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Create list_processes tool
pub fn create_list_processes_tool() -> MCPTool {
    let mut props = HashMap::new();

    props.insert(
        "status_filter".to_string(),
        enum_prop(
            vec!["all", "running", "finished"],
            "all",
            Some("Filter by status: 'all' (default), 'running', or 'finished'"),
        ),
    );

    MCPTool {
        name: "list_processes".to_string(),
        title: Some("List Processes".to_string()),
        description: "List all background processes in the current session. \
                      Filter by status: 'all' (default), 'running', or 'finished'."
            .to_string(),
        input_schema: object_schema(props, Vec::new()),
        output_schema: None,
        annotations: None,
    }
}
