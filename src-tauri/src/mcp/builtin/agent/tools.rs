use serde_json::json;

use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

pub fn all_tools() -> Vec<MCPTool> {
    vec![
        create_tool(),
        list_tool(),
        update_tool(),
        create_org_tool(),
        get_org_tool(),
        start_session_tool(),
        message_to_session_tool(),
        check_session_tool(),
        compact_session_context_tool(),
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
                ("description".to_string(), string_prop(None, None, Some("Short description of what this agent does. If omitted, the configuration is created without a description."))),
                ("systemPrompt".to_string(), string_prop(None, None, Some("The core personality and instructions for the agent. If omitted, no custom system prompt is stored."))),
                ("temperature".to_string(), number_prop(Some(0.0), Some(2.0), Some("Sampling temperature (0.0 to 2.0). If omitted, the configuration leaves temperature unset and the runtime/model default applies."))),
                ("builtinCapabilities".to_string(), array_schema(string_prop(None, None, None), Some("List of builtin service aliases to allow (e.g. ['workspace', 'browser', 'planning']). If omitted, the configuration leaves builtin capability overrides unset."))),
                ("externalMcpServers".to_string(), array_schema(string_prop(None, None, None), Some("List of external MCP server IDs to allow (e.g. ['github', 'google-search']). If omitted, the configuration leaves external MCP server overrides unset."))),
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
                (
                    "type".to_string(),
                    enum_prop(
                        vec!["configs", "sessions"],
                        "configs",
                        Some("Whether to list agent configurations or delegated sessions."),
                    ),
                ),
                ("query".to_string(), string_prop(None, None, Some("Search term to filter agent configurations by name or description. If omitted, no text filtering is applied."))),
                (
                    "limit".to_string(),
                    integer_prop_with_default(
                        Some(1),
                        Some(100),
                        20,
                        Some("Maximum number of items to return."),
                    ),
                ),
                (
                    "offset".to_string(),
                    integer_prop_with_default(
                        Some(0),
                        None,
                        0,
                        Some("Pagination offset (0-based)."),
                    ),
                ),
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
                    string_prop(None, None, Some("New name for the agent. If omitted, keep the current name unchanged.")),
                ),
                (
                    "description".to_string(),
                    string_prop(None, None, Some("New description. If omitted, keep the current description unchanged.")),
                ),
                (
                    "systemPrompt".to_string(),
                    string_prop(None, None, Some("New system instructions. If omitted, keep the current system prompt unchanged.")),
                ),
                (
                    "temperature".to_string(),
                    number_prop(Some(0.0), Some(2.0), Some("Change temperature. If omitted, keep the current temperature unchanged.")),
                ),
                ("builtinCapabilities".to_string(), array_schema(string_prop(None, None, None), Some("Replace the allowed builtin service aliases. If omitted, keep the current builtin capability list unchanged."))),
                ("externalMcpServers".to_string(), array_schema(string_prop(None, None, None), Some("Replace the allowed external MCP server IDs. If omitted, keep the current external MCP server list unchanged."))),
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
        description: "Spawn a new child agent session to delegate a specific task. When the current session already belongs to an explicit org, the child joins that org automatically and, unless workspaceOverride is provided, shares the org root workspace by default. Set includeCurrentOrg=false to opt out. Returns immediately with session info unless waitForResult=true.".to_string(),
        input_schema: object_prop(
            vec![
                ("agentId".to_string(), string_prop_required("Exact agent configuration ID to use. Call `list(type='configs')` first, then copy the returned ID. Do not put the agent name here.")),
                ("task".to_string(), string_prop_required("The specific task description for the sub-agent.")),
                ("workspaceOverride".to_string(), string_prop(None, None, Some("Absolute workspace path for the child session. If omitted, a plain child uses its default isolated workspace; an org child inherits the explicit org root workspace by default."))),
                ("maxDepth".to_string(), integer_prop(Some(0), None, Some("Override the delegation depth limit for this child session. If omitted, inherit the caller's maxDepth when present; otherwise leave the depth limit unset."))),
                ("maxFanout".to_string(), integer_prop(Some(0), None, Some("Override the delegation fanout limit for this child session. If omitted, inherit the caller's maxFanout when present; otherwise leave the fanout limit unset."))),
                ("includeCurrentOrg".to_string(), boolean_prop(Some("Optional explicit override for org inheritance. When omitted, children automatically inherit the caller's explicit org membership; set false to keep a delegated child out of Org view."))),
                ("waitForResult".to_string(), {
                    let mut schema = boolean_prop(Some("If true, block until the session reaches a terminal result and return that final answer."));
                    schema.default = Some(json!(false));
                    schema
                }),
            ],
            vec!["agentId".to_string(), "task".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn create_org_tool() -> MCPTool {
    MCPTool {
        name: "createOrg".to_string(),
        title: Some("Create Explicit Org".to_string()),
        description: "Mark the current root session as an explicit org root. This is the only path that makes a lineage appear in Org view. Use this from a top-level/root session, not from arbitrary child sessions. If the teamwork scaffold is missing or inconsistent, the result will tell you to use teamwork next.".to_string(),
        input_schema: object_prop(
            vec![(
                "name".to_string(),
                string_prop_required("Human-readable org name to create from the current root session."),
            )],
            vec!["name".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn get_org_tool() -> MCPTool {
    MCPTool {
        name: "getOrg".to_string(),
        title: Some("Get Org Summary".to_string()),
        description: "Get the current explicit org summary, including root session and member sessions. If orgId is omitted, the current session's org is used.".to_string(),
        input_schema: object_prop(
            vec![(
                "orgId".to_string(),
                string_prop(None, None, Some("Optional explicit org ID. If omitted, uses the caller session's org.")),
            )],
            vec![],
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
            "Send a follow-up message or additional instructions to an existing sub-agent session to continue the conversation. This can also be used to explicitly wake paused or error sessions and retry the delegated workflow from the latest stable state."
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
        description: "Check the status of a sub-agent session or wait for it to complete. Returns the latest known status and turn count, preserves that progress metadata even when a blocking wait times out, and explicitly reports when a paused/error session needs recovery via messageToSession.".to_string(),
        input_schema: object_prop(
            vec![
                ("sessionId".to_string(), string_prop_required("ID of the session to check.")),
                ("wait".to_string(), {
                    let mut schema = boolean_prop(Some("If true, block until the session reaches a terminal state."));
                    schema.default = Some(json!(false));
                    schema
                }),
                (
                    "timeout".to_string(),
                    integer_prop_with_default(
                        Some(1),
                        Some(3600),
                        3600,
                        Some("Maximum seconds to wait when `wait` is true. Ignored when `wait` is false."),
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

fn compact_session_context_tool() -> MCPTool {
    MCPTool {
        name: "compactSessionContext".to_string(),
        title: Some("Compact Another Session Context".to_string()),
        description: "Force a compaction pass for another active delegated session and wait for the compact summary result. Cross-session only: you cannot compact the current session with this tool. Use it when a child session has accumulated too much history and you want to refresh its stored compact summary before sending more work.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "sessionId".to_string(),
                    string_prop_required("ID of the target delegated session to compact. Must not be the current session."),
                ),
                (
                    "timeout".to_string(),
                    integer_prop_with_default(
                        Some(5),
                        Some(300),
                        60,
                        Some("Maximum seconds to wait for the compaction to finish and return the new compact summary."),
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
