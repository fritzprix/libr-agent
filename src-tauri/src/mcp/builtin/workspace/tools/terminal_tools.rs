use crate::mcp::{utils::schema_builder::*, MCPTool};
use std::collections::HashMap;

pub fn create_open_terminal_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "command".to_string(),
        string_prop(Some(1), Some(1000), Some("The command to execute.")),
    );
    props.insert(
        "args".to_string(),
        array_schema(
            string_prop(Some(1), Some(1000), None),
            Some("An array of arguments for the command."),
        ),
    );
    props.insert(
        "working_dir".to_string(),
        string_prop(
            Some(1),
            Some(1000),
            Some("The working directory to run the command in. Defaults to the session workspace."),
        ),
    );
    props.insert("env".to_string(), object_schema(HashMap::new(), vec![]));
    props.insert(
        "isolation".to_string(),
        string_prop(
            None,
            None,
            Some("The isolation level for the command: 'basic', 'medium' (default), or 'high'."),
        ),
    );

    MCPTool {
        name: "open_terminal".to_string(),
        title: Some("Open Terminal".to_string()),
        description: "Opens a new terminal session and executes a command asynchronously. Returns a terminal ID.".to_string(),
        input_schema: object_schema(props, vec!["command".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_close_terminal_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "terminal_id".to_string(),
        string_prop(Some(1), Some(100), Some("The ID of the terminal to close.")),
    );

    MCPTool {
        name: "close_terminal".to_string(),
        title: Some("Close Terminal".to_string()),
        description: "Closes an active terminal session and terminates its process.".to_string(),
        input_schema: object_schema(props, vec!["terminal_id".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_read_terminal_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "terminal_id".to_string(),
        string_prop(
            Some(1),
            Some(100),
            Some("The ID of the terminal to read from."),
        ),
    );
    props.insert(
        "from_seq".to_string(),
        integer_prop(
            Some(0),
            None,
            Some("The sequence number to start reading from. Defaults to 0 (the beginning)."),
        ),
    );

    MCPTool {
        name: "read_terminal".to_string(),
        title: Some("Read Terminal Output".to_string()),
        description:
            "Reads output from an active terminal session, starting from a given sequence number."
                .to_string(),
        input_schema: object_schema(props, vec!["terminal_id".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_list_terminals_tool() -> MCPTool {
    MCPTool {
        name: "list_terminals".to_string(),
        title: Some("List Terminals".to_string()),
        description: "Lists all active terminal sessions for the current agent session."
            .to_string(),
        input_schema: object_schema(HashMap::new(), vec![]),
        output_schema: None,
        annotations: None,
    }
}
