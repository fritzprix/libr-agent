use crate::mcp::{
    schema::SchemaProperties, utils::schema_builder::*, wait_extension::LibragentWaitExtension,
    MCPTool,
};

const PROCESS_ID_PROP: &str = "Process ID from workspace__spawnProcess, a sync-timeout handoff response, or workspace__listProcesses. Never invent an ID.";

/// Create read_process_output tool
pub fn create_read_process_output_tool() -> MCPTool {
    let mut props = SchemaProperties::new();

    props.insert(
        "processId".to_string(),
        string_prop_required(PROCESS_ID_PROP),
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
            "Read captured stdout, stderr, or both from a background process ID (workspace__spawnProcess or sync-timeout handoff). Works while the process is still running and after it finishes. Not for synchronous isolated/shell commands that already returned stdout/stderr inline — those registry entries are removed immediately. Returns output_paths for diagnostics (absolute internal paths; do not pass them to workspace__readFile/workspace__listDirectory)."
                .to_string(),
        input_schema: object_schema(props, vec!["processId".to_string(), "stream".to_string()]),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}

/// Create wait_for_process tool
pub fn create_wait_for_process_tool() -> MCPTool {
    let mut props = SchemaProperties::new();

    props.insert(
        "processId".to_string(),
        string_prop_required(PROCESS_ID_PROP),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(0),
            Some(3600),
            30,
            Some(
                "Maximum wait in seconds before returning. Use 0 to return current status immediately.",
            ),
        ),
    );

    MCPTool {
        name: "waitForProcess".to_string(),
        title: Some("Wait For Process".to_string()),
        description: "Block until a background process finishes or times out. Requires a processId from workspace__spawnProcess, a sync-timeout handoff, or workspace__listProcesses — not for completed sync workspace__runShell/workspace__runPowerShell results (those have no processId)."
            .to_string(),
        input_schema: object_schema(props, vec!["processId".to_string()]),
        output_schema: None,
        annotations: None,
        libragent_wait: Some(LibragentWaitExtension::wait_for_process()),
    }
}

/// Create list_processes tool
pub fn create_list_processes_tool() -> MCPTool {
    let mut props = SchemaProperties::new();

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
        libragent_wait: None,
    }
}

/// Create stop_process tool
pub fn create_stop_process_tool() -> MCPTool {
    let mut props = SchemaProperties::new();

    props.insert(
        "processId".to_string(),
        string_prop_required(PROCESS_ID_PROP),
    );

    MCPTool {
        name: "stopProcess".to_string(),
        title: Some("Stop Process".to_string()),
        description: "Terminate a running background process immediately. Requires a processId from workspace__spawnProcess, a sync-timeout handoff, or workspace__listProcesses."
            .to_string(),
        input_schema: object_schema(props, vec!["processId".to_string()]),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}
