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
        terminate_session_tool(),
        list_assistants_tool(),
        get_assistant_tool(),
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
        name: "spawnAgent".to_string(),
        title: Some("Spawn Agent".to_string()),
        description: "Spawn a child agent with a specific task. Set awaitCompletion=true to block until the child finishes and return its final result in a single call. With awaitCompletion=false (default) the call returns immediately and you must call awaitAgent separately."
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "parentSessionId".to_string(),
                    string_prop(None, None, Some("Optional parent session ID. Ignored in caller context; required only when no caller context exists")),
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
                (
                    "awaitCompletion".to_string(),
                    boolean_prop(Some("If true, block until the child session reaches a terminal state and return its final result. Default: false")),
                ),
                (
                    "timeoutSeconds".to_string(),
                    integer_prop(
                        Some(1),
                        None,
                        Some("Maximum seconds to wait when awaitCompletion=true. Default: 180"),
                    ),
                ),
                (
                    "includeLastAssistantMessage".to_string(),
                    boolean_prop(Some("When awaitCompletion=true, include last assistant message text in the result. Default: true")),
                ),
                (
                    "resultMessageLimit".to_string(),
                    integer_prop(
                        Some(1),
                        Some(200),
                        Some("Max number of messages to return when awaitCompletion=true. Default: 20"),
                    ),
                ),
                (
                    "assistantMessageMaxChars".to_string(),
                    integer_prop(
                        Some(1),
                        Some(200000),
                        Some("Truncate returned assistant message text to this length. Default: no limit"),
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
        name: "getAgentStatus".to_string(),
        title: Some("Get Agent Status".to_string()),
        description: "Get current status and metadata of an agent session.".to_string(),
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
        name: "awaitAgent".to_string(),
        title: Some("Await Agent".to_string()),
        description: "Wait until an agent finishes its task, then return its final result. Prefer this over polling getAgentLog repeatedly."
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
        name: "getAgentLog".to_string(),
        title: Some("Get Agent Log".to_string()),
        description: "Fetch recent messages from an agent with context-budget controls. Use awaitAgent instead if you just want the final result.".to_string(),
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
        name: "getChildAgents".to_string(),
        title: Some("Get Child Agents".to_string()),
        description: "List all agents directly spawned by a parent agent.".to_string(),
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
        name: "messageAgent".to_string(),
        title: Some("Message Agent".to_string()),
        description:
            "Send a user message to a running agent. If the agent is busy, the message will be queued."
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
        name: "terminateAgent".to_string(),
        title: Some("Terminate Agent".to_string()),
        description: "Terminate an agent session immediately.".to_string(),
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
        name: "listAgentTypes".to_string(),
        title: Some("List Agent Types".to_string()),
        description: "List available agent types (assistants) you can spawn. Call this to discover which specialists exist before delegating tasks.".to_string(),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

pub fn get_assistant_tool() -> MCPTool {
    MCPTool {
        name: "getAgentConfig".to_string(),
        title: Some("Get Agent Config".to_string()),
        description: "Get the full configuration of an agent type (system prompt, tools, capabilities). Use for capability verification before spawning.".to_string(),
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
