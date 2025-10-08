use crate::mcp::{utils::schema_builder::*, MCPTool};
use serde_json::json;
use std::collections::HashMap;

use super::super::utils::constants::{
    DEFAULT_EXECUTION_TIMEOUT, MAX_CODE_SIZE, MAX_EXECUTION_TIMEOUT,
};

pub fn create_execute_python_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "terminal_id".to_string(),
        string_prop(Some(1), Some(256), Some("ID of the terminal session")),
    );
    props.insert(
        "code".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(MAX_CODE_SIZE as u32),
            Some("Python code to execute in the terminal"),
            vec![json!("print('Hello, World!')")],
        ),
    );
    props.insert(
        "async".to_string(),
        boolean_prop_with_default(
            false,
            Some("If true, execute asynchronously and return immediately"),
        ),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(1),
            Some(MAX_EXECUTION_TIMEOUT as i64),
            DEFAULT_EXECUTION_TIMEOUT as i64,
            Some("Timeout in seconds (default: 30, only for sync execution)"),
        ),
    );

    MCPTool {
        name: "execute_python".to_string(),
        title: Some("Execute Python Code in Terminal".to_string()),
        description: "Executes Python code within a specified terminal session.".to_string(),
        input_schema: object_schema(props, vec!["terminal_id".to_string(), "code".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_execute_typescript_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "terminal_id".to_string(),
        string_prop(Some(1), Some(256), Some("ID of the terminal session")),
    );
    props.insert(
        "code".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(MAX_CODE_SIZE as u32),
            Some("TypeScript code to execute in the terminal"),
            vec![json!("console.log('Hello, World!');")],
        ),
    );
    props.insert(
        "async".to_string(),
        boolean_prop_with_default(
            false,
            Some("If true, execute asynchronously and return immediately"),
        ),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(1),
            Some(MAX_EXECUTION_TIMEOUT as i64),
            DEFAULT_EXECUTION_TIMEOUT as i64,
            Some("Timeout in seconds (default: 30, only for sync execution)"),
        ),
    );

    MCPTool {
        name: "execute_typescript".to_string(),
        title: Some("Execute TypeScript Code in Terminal".to_string()),
        description: "Executes TypeScript code within a specified terminal session using Deno."
            .to_string(),
        input_schema: object_schema(props, vec!["terminal_id".to_string(), "code".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_execute_shell_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "terminal_id".to_string(),
        string_prop(Some(1), Some(256), Some("ID of the terminal session")),
    );
    props.insert(
        "command".to_string(),
        string_prop_with_examples(
            Some(1),
            Some(1000),
            Some("Shell command to execute in the terminal"),
            vec![json!("ls -la"), json!("grep -r 'pattern' .")],
        ),
    );
    props.insert(
        "async".to_string(),
        boolean_prop_with_default(
            false,
            Some("If true, execute asynchronously and return immediately"),
        ),
    );
    props.insert(
        "timeout".to_string(),
        integer_prop_with_default(
            Some(1),
            Some(MAX_EXECUTION_TIMEOUT as i64),
            DEFAULT_EXECUTION_TIMEOUT as i64,
            Some("Timeout in seconds (default: 30, only for sync execution)"),
        ),
    );

    MCPTool {
        name: "execute_shell".to_string(),
        title: Some("Execute Shell Command in Terminal".to_string()),
        description: "Executes a shell command within a specified terminal session.".to_string(),
        input_schema: object_schema(
            props,
            vec!["terminal_id".to_string(), "command".to_string()],
        ),
        output_schema: None,
        annotations: None,
    }
}

// New Terminal Tools
pub fn create_open_new_terminal_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "shell".to_string(),
        string_prop(
            Some(1),
            Some(256),
            Some("Optional shell executable (e.g., 'bash', 'zsh'). Defaults to system default."),
        ),
    );
    props.insert(
        "env".to_string(),
        object_prop(
            Some("Environment variables to set for the terminal session."),
            HashMap::new(),
        ),
    );
    MCPTool {
        name: "open_new_terminal".to_string(),
        title: Some("Open New Terminal".to_string()),
        description: "Opens a new persistent terminal session and returns its ID.".to_string(),
        input_schema: object_schema(props, vec![]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_read_terminal_output_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "terminal_id".to_string(),
        string_prop(
            Some(1),
            Some(256),
            Some("ID of the terminal session to read from"),
        ),
    );
    props.insert(
        "since_index".to_string(),
        integer_prop(
            Some(0),
            None,
            Some("The index to start reading from. If omitted, reads all available output."),
        ),
    );
    MCPTool {
        name: "read_terminal_output".to_string(),
        title: Some("Read Terminal Output".to_string()),
        description: "Reads new output from a terminal session since the last read.".to_string(),
        input_schema: object_schema(props, vec!["terminal_id".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_close_terminal_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "terminal_id".to_string(),
        string_prop(
            Some(1),
            Some(256),
            Some("ID of the terminal session to close"),
        ),
    );
    MCPTool {
        name: "close_terminal".to_string(),
        title: Some("Close Terminal".to_string()),
        description: "Closes an active terminal session.".to_string(),
        input_schema: object_schema(props, vec!["terminal_id".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_list_terminals_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "session_id".to_string(),
        string_prop(
            Some(1),
            Some(256),
            Some("Optional session ID to filter terminals."),
        ),
    );
    MCPTool {
        name: "list_terminals".to_string(),
        title: Some("List Active Terminals".to_string()),
        description: "Lists all active terminal sessions, optionally filtered by session ID."
            .to_string(),
        input_schema: object_schema(props, vec![]),
        output_schema: None,
        annotations: None,
    }
}
