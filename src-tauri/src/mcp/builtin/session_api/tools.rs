use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

pub fn all_tools() -> Vec<MCPTool> {
    vec![
        health_check_tool(),
        create_child_session_tool(),
        get_session_tool(),
        wait_for_session_idle_tool(),
        get_child_sessions_tool(),
        get_messages_tool(),
        send_message_tool(),
        list_assistants_tool(),
        get_assistant_tool(),
        terminate_session_tool(),
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

pub fn create_child_session_tool() -> MCPTool {
    MCPTool {
        name: "createChildSession".to_string(),
        title: Some("Create Child Session".to_string()),
        description: "Spawn a new sub-agent worker to handle a specific task. Returns the new session ID immediately (non-blocking). Use this to delegate work in parallel."
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "parentSessionId".to_string(),
                    string_prop(None, None, Some("Parent session ID. If called from within a session, this is optional (defaults to self).")),
                ),
                (
                    "assistantId".to_string(),
                    string_prop_required("Assistant ID to bind to child session (e.g., 'assistant', 'coder')"),
                ),
                (
                    "request".to_string(),
                    string_prop_required("The task instruction for the sub-agent"),
                ),
                (
                    "name".to_string(),
                    string_prop(None, None, Some("Optional name for the sub-agent session (for your reference)")),
                ),
                (
                    "workspacePath".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Optional: Override workspace path for the sub-agent"),
                    ),
                ),
                (
                    "maxDepth".to_string(),
                    integer_prop(
                        Some(0),
                        None,
                        Some("Optional: Max recursion depth (default: unlimited)"),
                    ),
                ),
                (
                    "maxFanout".to_string(),
                    integer_prop(
                        Some(1),
                        None,
                        Some("Optional: Max direct children limit (default: unlimited)"),
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
        description: "Get status and metadata for a specific session ID.".to_string(),
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
        description: "Block and wait for a sub-agent to finish its task. Returns the final result/answer once the session becomes Idle. Use this to synchronize and get the output of a delegated task."
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
                        Some("Max time to wait (default: 180s)"),
                    ),
                ),
                (
                    "pollIntervalSeconds".to_string(),
                    integer_prop(
                        Some(1),
                        Some(30),
                        Some("Polling interval (default: 3s)"),
                    ),
                ),
                (
                    "includeLastAssistantMessage".to_string(),
                    boolean_prop(Some("If true (default), returns the final answer text from the sub-agent")),
                ),
                (
                    "resultMessageLimit".to_string(),
                    integer_prop(
                        Some(1),
                        Some(200),
                        Some("How far back to check for the final answer (default: 20 messages)"),
                    ),
                ),
                (
                    "assistantMessageMaxChars".to_string(),
                    integer_prop(
                        Some(0),
                        Some(200000),
                        Some("Max chars for the returned answer (default: 0 = full text)"),
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
        description: "Fetch the conversation history of a session. Use 'summaryOnly=true' (default) to save tokens, or 'includeRawPreview=true' to see full code/text content.".to_string(),
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
                        Some("Max messages to fetch"),
                    ),
                ),
                (
                    "summaryOnly".to_string(),
                    boolean_prop(Some("If true (default), returns concise summaries. Set false for full message structure.")),
                ),
                (
                    "includeRawPreview".to_string(),
                    boolean_prop(Some("If true, includes full text/code content in the summary (costs more tokens). Recommended for coding tasks.")),
                ),
                (
                    "previewLimit".to_string(),
                    integer_prop(
                        Some(1),
                        Some(10),
                        Some("Number of recent messages to preview in summary mode (default: 3)"),
                    ),
                ),
                (
                    "skipIfUnchanged".to_string(),
                    boolean_prop(Some("If true (default), avoids returning data if nothing changed since last fetch")),
                ),
                (
                    "minIntervalSeconds".to_string(),
                    integer_prop(
                        Some(0),
                        Some(120),
                        Some("Throttle: Minimum seconds between polls (default: 5)"),
                    ),
                ),
                (
                    "forcedRestSeconds".to_string(),
                    integer_prop(
                        Some(0),
                        Some(300),
                        Some("Throttle: Cooldown after rapid polling (default: 20)"),
                    ),
                ),
                (
                    "rapidCallThreshold".to_string(),
                    integer_prop(
                        Some(2),
                        Some(10),
                        Some("Throttle: Rapid poll limit (default: 3)"),
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
        description: "List all direct sub-agents (workers) created by a specific session."
            .to_string(),
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

pub fn get_assistant_tool() -> MCPTool {
    MCPTool {
        name: "getAssistant".to_string(),
        title: Some("Get Assistant Details".to_string()),
        description: "Get detailed configuration of an assistant (system prompt, tools, model). Use this for meta-analysis or verifying capabilities.".to_string(),
        input_schema: object_prop(
            vec![(
                "assistantId".to_string(),
                string_prop_required("Target assistant ID"),
            )],
            vec!["assistantId".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}
