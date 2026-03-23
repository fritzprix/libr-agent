use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

pub fn all_tools() -> Vec<MCPTool> {
    vec![
        create_tool(),
        list_tool(),
        update_tool(),
        start_session_tool(),
        message_to_session_tool(),
        check_session_tool(),
        stop_session_tool(),
    ]
}

fn create_tool() -> MCPTool {
    MCPTool {
        name: "create".to_string(),
        title: Some("Create Agent Configuration".to_string()),
        description: "Create a new named agent configuration (assistant) with a specific system prompt, temperature, and tool capabilities. Model selection is controlled at session or global settings level, not here.".to_string(),
        input_schema: object_prop(
            vec![
                ("name".to_string(), string_prop_required("Unique name for the agent configuration.")),
                ("description".to_string(), string_prop(None, None, Some("Short description of what this agent does."))),
                ("systemPrompt".to_string(), string_prop(None, None, Some("The core personality and instructions for the agent."))),
                ("temperature".to_string(), number_prop(Some(0.0), Some(2.0), Some("Sampling temperature (0.0 to 2.0)."))),
                ("builtinCapabilities".to_string(), array_schema(string_prop(None, None, None), Some("List of builtin service aliases to allow (e.g. ['workspace', 'browser', 'planning'])."))),
                ("externalMcpServers".to_string(), array_schema(string_prop(None, None, None), Some("List of external MCP server IDs to allow (e.g. ['github', 'google-search'])."))),
            ],
            vec!["name".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn list_tool() -> MCPTool {
    MCPTool {
        name: "list".to_string(),
        title: Some("List Agents and Sessions".to_string()),
        description: "List available agent configurations or active sub-agent sessions. Use this to discover specialized agents by name or description.".to_string(),
        input_schema: object_prop(
            vec![
                ("type".to_string(), string_prop(None, None, Some("What to list: 'configs' (default) or 'sessions' (sub-agents of current session)."))),
                ("query".to_string(), string_prop(None, None, Some("Optional search term to filter agent configurations by name or description."))),
                ("limit".to_string(), integer_prop(Some(1), Some(100), Some("Maximum number of items to return."))),
                ("offset".to_string(), integer_prop(Some(0), None, Some("Pagination offset."))),
            ],
            vec![],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn update_tool() -> MCPTool {
    MCPTool {
        name: "update".to_string(),
        title: Some("Update Agent Configuration".to_string()),
        description: "Update an existing agent configuration (assistant) including its system prompt, temperature, and tool access. Model selection is controlled elsewhere.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "id".to_string(),
                    string_prop_required("The unique ID of the agent configuration to update."),
                ),
                (
                    "name".to_string(),
                    string_prop(None, None, Some("New name for the agent.")),
                ),
                (
                    "description".to_string(),
                    string_prop(None, None, Some("New description.")),
                ),
                (
                    "systemPrompt".to_string(),
                    string_prop(None, None, Some("New system instructions.")),
                ),
                (
                    "temperature".to_string(),
                    number_prop(Some(0.0), Some(2.0), Some("Change temperature.")),
                ),
                ("builtinCapabilities".to_string(), array_schema(string_prop(None, None, None), Some("Update allowed builtin service aliases."))),
                ("externalMcpServers".to_string(), array_schema(string_prop(None, None, None), Some("Update allowed external MCP server IDs."))),
            ],
            vec!["id".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn start_session_tool() -> MCPTool {
    MCPTool {
        name: "startSession".to_string(),
        title: Some("Start Agent Session".to_string()),
        description: "Spawn a new sub-agent session to delegate a specific task. Returns immediately with session info. Use checkSession to wait for the result.".to_string(),
        input_schema: object_prop(
            vec![
                ("agentId".to_string(), string_prop_required("Exact agent configuration ID to use. Call `list(type='configs')` first, then copy the returned ID. Do not put the agent name here.")),
                ("task".to_string(), string_prop_required("The specific task description for the sub-agent.")),
                ("workspaceOverride".to_string(), string_prop(None, None, Some("Optional absolute workspace path for the child session. Use this when the sub-agent must operate in a specific workspace instead of its default isolated workspace."))),
                ("waitForResult".to_string(), boolean_prop(Some("If true, blocks until the agent finishes and returns the final answer (max wait: 1 hour). Default: false."))),
            ],
            vec!["agentId".to_string(), "task".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn message_to_session_tool() -> MCPTool {
    MCPTool {
        name: "messageToSession".to_string(),
        title: Some("Message Agent Session".to_string()),
        description:
            "Send a follow-up message or additional instructions to an existing sub-agent session to continue the conversation. You can send messages to sessions that have finished their previous tasks."
                .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "sessionId".to_string(),
                    string_prop_required("ID of the target sub-agent session."),
                ),
                (
                    "message".to_string(),
                    string_prop_required("The message or instruction to send."),
                ),
            ],
            vec!["sessionId".to_string(), "message".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn check_session_tool() -> MCPTool {
    MCPTool {
        name: "checkSession".to_string(),
        title: Some("Check Session Status".to_string()),
        description: "Check the status of a sub-agent session or wait for it to complete. Returns the current state or the final answer if finished.".to_string(),
        input_schema: object_prop(
            vec![
                ("sessionId".to_string(), string_prop_required("ID of the session to check.")),
                ("wait".to_string(), boolean_prop(Some("If true, blocks until the session reaches a terminal state (finished/error)."))),
                ("timeout".to_string(), integer_prop(Some(1), Some(3600), Some("Maximum seconds to wait if 'wait' is true (default: 3600)."))),
            ],
            vec!["sessionId".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn stop_session_tool() -> MCPTool {
    MCPTool {
        name: "stopSession".to_string(),
        title: Some("Stop Agent Session".to_string()),
        description: "Forcefully terminate an active sub-agent session. Use this when a delegated task is no longer needed or if the sub-agent appears stuck. This immediately halts execution.".to_string(),
        input_schema: object_prop(
            vec![
                ("sessionId".to_string(), string_prop_required("ID of the session to stop.")),
            ],
            vec!["sessionId".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}
