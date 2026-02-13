use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

pub fn all_tools() -> Vec<MCPTool> {
    vec![
        health_check_tool(),
        create_session_tool(),
        get_session_tool(),
        get_messages_tool(),
        send_message_tool(),
        terminate_session_tool(),
        list_assistants_tool(),
    ]
}

pub fn health_check_tool() -> MCPTool {
    MCPTool {
        name: "healthCheck".to_string(),
        title: Some("Health Check".to_string()),
        description: "Check whether the internal session HTTP API is reachable.".to_string(),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_session_tool() -> MCPTool {
    MCPTool {
        name: "createSession".to_string(),
        title: Some("Create Session".to_string()),
        description: "Create a new agent session through the internal Session API. Returns session ID for follow-up calls.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "assistantId".to_string(),
                    string_prop_required("Assistant ID to bind to the new session"),
                ),
                (
                    "request".to_string(),
                    string_prop_required("Initial user request to start workflow"),
                ),
                (
                    "name".to_string(),
                    string_prop(None, None, Some("Optional session display name")),
                ),
                (
                    "workspacePath".to_string(),
                    string_prop(None, None, Some("Optional absolute workspace override path")),
                ),
            ],
            vec!["assistantId".to_string(), "request".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn get_session_tool() -> MCPTool {
    MCPTool {
        name: "getSession".to_string(),
        title: Some("Get Session".to_string()),
        description: "Get current session metadata/status by session ID.".to_string(),
        input_schema: object_prop(
            vec![(
                "sessionId".to_string(),
                string_prop_required("Target session ID"),
            )],
            vec!["sessionId".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn get_messages_tool() -> MCPTool {
    MCPTool {
        name: "getMessages".to_string(),
        title: Some("Get Messages".to_string()),
        description: "Fetch recent messages for a session.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "sessionId".to_string(),
                    string_prop_required("Target session ID"),
                ),
                (
                    "limit".to_string(),
                    integer_prop(
                        Some(1),
                        Some(500),
                        Some("Optional maximum messages to fetch"),
                    ),
                ),
            ],
            vec!["sessionId".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn send_message_tool() -> MCPTool {
    MCPTool {
        name: "sendMessage".to_string(),
        title: Some("Send Message".to_string()),
        description:
            "Send a new user message to a session. If session is busy, message may be queued."
                .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "sessionId".to_string(),
                    string_prop_required("Target session ID"),
                ),
                (
                    "content".to_string(),
                    string_prop_required("User message content"),
                ),
            ],
            vec!["sessionId".to_string(), "content".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn terminate_session_tool() -> MCPTool {
    MCPTool {
        name: "terminateSession".to_string(),
        title: Some("Terminate Session".to_string()),
        description: "Terminate a session immediately.".to_string(),
        input_schema: object_prop(
            vec![(
                "sessionId".to_string(),
                string_prop_required("Target session ID"),
            )],
            vec!["sessionId".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn list_assistants_tool() -> MCPTool {
    MCPTool {
        name: "listAssistants".to_string(),
        title: Some("List Assistants".to_string()),
        description: "List all available assistants from the internal Session API.".to_string(),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}
