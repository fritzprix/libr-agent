use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

pub fn all_tools() -> Vec<MCPTool> {
    vec![
        health_check_tool(),
        create_session_tool(),
        create_child_session_tool(),
        get_session_tool(),
        wait_for_session_idle_tool(),
        get_child_sessions_tool(),
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
        description: "Create a new agent session through the internal Session API. If parentSessionId is omitted and this tool is called inside a session, caller session ID is automatically used as parent for lineage tracking. Returns session ID for follow-up calls.".to_string(),
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
                (
                    "maxDepth".to_string(),
                    integer_prop(Some(0), None, Some("Optional recursion depth limit (None = unlimited)")),
                ),
                (
                    "maxFanout".to_string(),
                    integer_prop(
                        Some(1),
                        None,
                        Some("Optional max direct children per parent session (None = unlimited)"),
                    ),
                ),
                (
                    "parentSessionId".to_string(),
                    string_prop(None, None, Some("Optional parent session ID for lineage tracking")),
                ),
            ],
            vec!["assistantId".to_string(), "request".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

pub fn create_child_session_tool() -> MCPTool {
    MCPTool {
        name: "createChildSession".to_string(),
        title: Some("Create Child Session".to_string()),
        description: "Create a child session linked to a parent session (lineage contract). If parentSessionId is omitted, caller session ID is used automatically. The alias 'current' is also supported."
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "parentSessionId".to_string(),
                    string_prop(None, None, Some("Optional parent session ID. If omitted or set to 'current', caller session ID is used")),
                ),
                (
                    "assistantId".to_string(),
                    string_prop_required("Assistant ID to bind to child session"),
                ),
                (
                    "request".to_string(),
                    string_prop_required("Initial request for child session"),
                ),
                (
                    "name".to_string(),
                    string_prop(None, None, Some("Optional child session name")),
                ),
                (
                    "workspacePath".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Optional absolute workspace override path"),
                    ),
                ),
                (
                    "maxDepth".to_string(),
                    integer_prop(
                        Some(0),
                        None,
                        Some("Optional recursion depth limit (None = unlimited)"),
                    ),
                ),
                (
                    "maxFanout".to_string(),
                    integer_prop(
                        Some(1),
                        None,
                        Some("Optional max direct children per parent session (None = unlimited)"),
                    ),
                ),
            ],
            vec![
                "assistantId".to_string(),
                "request".to_string(),
            ],
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

pub fn wait_for_session_idle_tool() -> MCPTool {
    MCPTool {
        name: "waitForSessionIdle".to_string(),
        title: Some("Wait For Session Idle".to_string()),
        description: "Wait until a session reaches terminal status (usually Idle), then optionally fetch latest assistant result. By default, returns full assistant text in the text channel. Use this instead of aggressive getMessages polling."
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "sessionId".to_string(),
                    string_prop_required("Target session ID"),
                ),
                (
                    "timeoutSeconds".to_string(),
                    integer_prop(
                        Some(5),
                        Some(900),
                        Some("Max time to wait before timeout (default: 180)"),
                    ),
                ),
                (
                    "pollIntervalSeconds".to_string(),
                    integer_prop(
                        Some(1),
                        Some(30),
                        Some("Polling interval while waiting (default: 3)"),
                    ),
                ),
                (
                    "includeLastAssistantMessage".to_string(),
                    boolean_prop(Some("If true (default), include latest assistant text result after session becomes idle")),
                ),
                (
                    "resultMessageLimit".to_string(),
                    integer_prop(
                        Some(1),
                        Some(200),
                        Some("How many latest messages to inspect when extracting final assistant result (default: 20)"),
                    ),
                ),
                (
                    "assistantMessageMaxChars".to_string(),
                    integer_prop(
                        Some(0),
                        Some(200000),
                        Some("Optional max chars for returned assistant text (default: 0 = full text)"),
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

pub fn get_messages_tool() -> MCPTool {
    MCPTool {
        name: "getMessages".to_string(),
        title: Some("Get Messages".to_string()),
        description: "Fetch recent messages for a session with context-budget controls (summary mode, preview limit, and unchanged-result dedupe).".to_string(),
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
                (
                    "summaryOnly".to_string(),
                    boolean_prop(Some("If true (default), return concise previews instead of expanded message lines")),
                ),
                (
                    "includeRawPreview".to_string(),
                    boolean_prop(Some("If true, include longer text snippets in previews (higher token cost)")),
                ),
                (
                    "previewLimit".to_string(),
                    integer_prop(
                        Some(1),
                        Some(10),
                        Some("Maximum number of message previews to include in text output (default: 3)"),
                    ),
                ),
                (
                    "skipIfUnchanged".to_string(),
                    boolean_prop(Some("If true (default), return a short notice when fetched message digest is unchanged since last fetch")),
                ),
                (
                    "minIntervalSeconds".to_string(),
                    integer_prop(
                        Some(0),
                        Some(120),
                        Some("Minimum seconds between repeated polls for the same caller/session/limit key (default: 5; set 0 to disable)"),
                    ),
                ),
                (
                    "forcedRestSeconds".to_string(),
                    integer_prop(
                        Some(0),
                        Some(300),
                        Some("Hard cooldown seconds applied after too many rapid polls (default: 20; set 0 to disable)"),
                    ),
                ),
                (
                    "rapidCallThreshold".to_string(),
                    integer_prop(
                        Some(2),
                        Some(10),
                        Some("Rapid poll count threshold that triggers forced cooldown (default: 3)"),
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

pub fn get_child_sessions_tool() -> MCPTool {
    MCPTool {
        name: "getChildSessions".to_string(),
        title: Some("Get Child Sessions".to_string()),
        description: "List direct child sessions for a parent session.".to_string(),
        input_schema: object_prop(
            vec![(
                "parentSessionId".to_string(),
                string_prop_required("Parent session ID"),
            )],
            vec!["parentSessionId".to_string()],
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
