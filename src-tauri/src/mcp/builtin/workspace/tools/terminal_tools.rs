use crate::define_mcp_tool;

define_mcp_tool! {
    const POLL_PROCESS = "pollProcess";
    fn create_poll_process_tool();
    title: "Poll Process Status";
    description: "Check the status of an asynchronously running process. \
                  Optionally retrieve the last N lines of output (max 100 lines). \
                  Only processes from the current session can be queried.";
    inputs: props => {
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
    };
    required: vec!["processId".to_string()];
}

define_mcp_tool! {
    const READ_PROCESS_OUTPUT = "readProcessOutput";
    fn create_read_process_output_tool();
    title: "Read Process Output";
    description: "Read stdout or stderr from a background process. \
                  TEXT OUTPUT ONLY. Maximum 100 lines per request. \
                  Use 'tail' mode for last N lines, 'head' for first N lines.";
    inputs: props => {
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
    };
    required: vec!["processId".to_string(), "stream".to_string()];
}

define_mcp_tool! {
    const LIST_PROCESSES = "listProcesses";
    fn create_list_processes_tool();
    title: "List Processes";
    description: "List all background processes in the current session. \
                  Filter by status: 'all' (default), 'running', or 'finished' (includes failed).";
    inputs: props => {
        props.insert(
            "statusFilter".to_string(),
            enum_prop(
                vec!["all", "running", "finished"],
                "all",
                Some("Filter by status: 'all' (default), 'running', or 'finished'"),
            ),
        );
    };
    required: vec![];
}

define_mcp_tool! {
    const STOP_PROCESS = "stopProcess";
    fn create_stop_process_tool();
    title: "Stop Process";
    description: "Stop a running background process. \
                  Sends a termination signal to the process.";
    inputs: props => {
        props.insert(
            "processId".to_string(),
            string_prop_required("Process ID to stop"),
        );
    };
    required: vec!["processId".to_string()];
}
