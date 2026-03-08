use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

pub fn all_tools() -> Vec<MCPTool> {
    vec![
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
        description: "Spawn a NEW child agent with a specific task. To send a message to an EXISTING agent, use the messageAgent tool instead! Set awaitCompletion=true (default) to block until the child finishes and return its final result in a single call — results flow back to parent context (DFS-style). Set awaitCompletion=false to return immediately and poll with awaitAgent separately."
            .to_string(),
        input_schema: object_prop(
            vec![
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
                        Some("Recursion depth limit. Omit for no limit."),
                    ),
                ),
                (
                    "maxFanout".to_string(),
                    integer_prop(
                        Some(1),
                        None,
                        Some("Max direct children per parent session. Omit for no limit."),
                    ),
                ),
                (
                    "awaitCompletion".to_string(),
                    boolean_prop(Some("If true (default), block until the child finishes and return its final result. Set false to spawn and return immediately — then use awaitAgent(sessionId) to collect results.")),
                ),
                (
                    "timeoutSeconds".to_string(),
                    integer_prop(
                        Some(1),
                        None,
                        Some("Max seconds to wait when awaitCompletion=true. Default: 180"),
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
        description: "Get current status and metadata of an agent session. Possible statuses: busy (working), idle (completed successfully), paused (waiting for resume), terminated (manually stopped), failed, error. Use awaitAgent instead if you want to block until completion.".to_string(),
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
        description: "Wait until an agent finishes its task, then return its final result. Uses push notifications — no polling delay. Terminal states: idle (success), terminated (manual stop), failed, error. Prefer this over polling getAgentLog repeatedly."
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
                        Some(1),
                        Some(200000),
                        Some("Max chars for returned assistant text. Omit for full text."),
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
        description: "List all agents directly spawned by the calling session.".to_string(),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

pub fn send_message_tool() -> MCPTool {
    MCPTool {
        name: "messageAgent".to_string(),
        title: Some("Message Agent".to_string()),
        description:
            "Send a user message to a running agent. If the agent is busy, the message will be queued. After sending, use awaitAgent(sessionId) to wait for the response."
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
