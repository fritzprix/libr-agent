use crate::mcp::builtin::tool_description::tool_description;
use serde_json::json;

use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

pub fn all_tools() -> Vec<MCPTool> {
    vec![
        create_tool(),
        list_tool(),
        update_tool(),
        prepare_teamwork_workspace_tool(),
        create_org_tool(),
        get_org_tool(),
        start_session_tool(),
        message_to_session_tool(),
        check_session_tool(),
        compact_session_context_tool(),
        stop_session_tool(),
        delete_session_tool(),
    ]
}

fn create_tool() -> MCPTool {
    MCPTool {
        name: "createAgent".to_string(),
        title: Some("Create Agent Configuration".to_string()),
        description: tool_description(
            "Create a new named agent configuration (assistant) with system prompt, temperature, and tool capabilities.",
            &[],
            &[
                "Choose a unique name and optional description.",
                "Set systemPrompt, temperature, and tool access lists as needed.",
                "Model selection is controlled at session or global settings — not here.",
            ],
            &[
                "Discover configs with agent__listAgents(type='configs').",
                "Spawn sessions with agent__startSession using the returned ID.",
            ],
        ),
        input_schema: object_prop(
            vec![
                ("name".to_string(), string_prop_required("Unique name for the agent configuration.")),
                ("description".to_string(), string_prop(None, None, Some("Short description of what this agent does. If omitted, the configuration is created without a description."))),
                ("temperature".to_string(), number_prop(Some(0.0), Some(2.0), Some("Sampling temperature (0.0 to 2.0). If omitted, the configuration leaves temperature unset and the runtime/model default applies."))),
                ("builtinCapabilities".to_string(), array_schema(string_prop(None, None, None), Some("List of optional builtin service aliases to add beyond the always-on core services (e.g. ['planning', 'browser', 'knowledge']). Core services remain enabled even when you pass a restricted list. If omitted (or []), only core services are enabled — list optional aliases explicitly when needed."))),
                ("externalMcpServers".to_string(), array_schema(string_prop(None, None, None), Some("List of external MCP server IDs to allow (e.g. ['github', 'google-search']). If omitted, the configuration leaves external MCP server overrides unset."))),
                ("systemPrompt".to_string(), string_prop(None, None, Some("The core personality and instructions for the agent. If omitted, no custom system prompt is stored."))),
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
        name: "listAgents".to_string(),
        title: Some("List Agents and Sessions".to_string()),
        description: tool_description(
            "List agent configurations or active delegated sub-agent sessions.",
            &[],
            &[
                "Set type='configs' for assistant definitions or type='sessions' for delegated sessions.",
                "Use query to filter configs by name or description.",
            ],
            &[
                "Start delegation with agent__startSession (configs) or agent__checkSession (sessions).",
                "Update configs with agent__updateAgent.",
            ],
        ),
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
                ("verbose".to_string(), {
                    let mut schema = boolean_prop(Some("If true, show full descriptions instead of truncating them in the text table."));
                    schema.default = Some(json!(false));
                    schema
                }),
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
        name: "updateAgent".to_string(),
        title: Some("Update Agent Configuration".to_string()),
        description: tool_description(
            "Update an existing agent configuration including system prompt, temperature, and tool access.",
            &["Configuration ID from agent__listAgents(type='configs')."],
            &[
                "Pass the config id and only the fields to change.",
                "Model selection is controlled elsewhere — not via this tool.",
            ],
            &[
                "Verify capabilities with agent__listAgents(verbose=true).",
                "Attach MCP servers using IDs from tool__listServers.",
            ],
        ),
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
                    "temperature".to_string(),
                    number_prop(Some(0.0), Some(2.0), Some("Change temperature. If omitted, keep the current temperature unchanged.")),
                ),
                ("builtinCapabilities".to_string(), array_schema(string_prop(None, None, None), Some("Replace the optional builtin service aliases that are added on top of the always-on core services. If omitted, keep the current optional builtin capability list unchanged."))),
                ("externalMcpServers".to_string(), array_schema(string_prop(None, None, None), Some("Replace the allowed external MCP server IDs. If omitted, keep the current external MCP server list unchanged."))),
                (
                    "systemPrompt".to_string(),
                    string_prop(None, None, Some("New system instructions. If omitted, keep the current system prompt unchanged.")),
                ),
            ],
            vec!["id".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn prepare_teamwork_workspace_tool() -> MCPTool {
    MCPTool {
        name: "prepareTeamworkWorkspace".to_string(),
        title: Some("Prepare Teamwork Artifact Directory".to_string()),
        description: tool_description(
            "Create or reuse an app-local teamwork artifact directory for the current governing/root session.",
            &["Caller should be a root or governing session."],
            &[
                "Call once when coordination metadata must live outside the repo workspace.",
                "This only prepares an empty @teamwork/ directory — it does not scaffold files and does not change the session workspace.",
            ],
            &[
                "Do not call prepareTeamworkWorkspace again after success.",
                "Scaffold next via the teamwork skill (prefer scripts/init_task_force.py with --output = response artifactPath) or write the full set under @teamwork/ including coordination/* and @teamwork/.libragent/teamwork.json.",
                "Only after the org scaffold is complete, call agent__createOrg, then agent__startSession for org members.",
            ],
        ),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

fn start_session_tool() -> MCPTool {
    MCPTool {
        name: "startSession".to_string(),
        title: Some("Start Agent Session".to_string()),
        description: tool_description(
            "Spawn a new child agent session to delegate a specific task.",
            &["Agent configuration ID from agent__listAgents(type='configs')."],
            &[
                "Pass agentId (config ID, not name) and a clear task description.",
                "Org children inherit org workspace by default unless workspaceOverride is set.",
                "Set waitForResult=true to block until the child finishes (optional timeout, default 3600s).",
            ],
            &[
                "Poll or wait with agent__checkSession.",
                "Send follow-ups with agent__messageToSession.",
            ],
        ),
        input_schema: object_prop(
            vec![
                ("agentId".to_string(), string_prop_required("Exact agent configuration ID to use. Call agent__listAgents(type='configs') first, then use the returned ID. Do not put the agent name here.")),
                ("workspaceOverride".to_string(), string_prop(None, None, Some("Absolute workspace path for the child session. If omitted, a plain child uses its default isolated workspace; an org child inherits the explicit org root workspace by default."))),
                ("waitForResult".to_string(), {
                    let mut schema = boolean_prop(Some("If true, block until the session reaches a terminal result and return that final answer. Uses timeout (default 3600s) as the maximum wait."));
                    schema.default = Some(json!(false));
                    schema
                }),
                (
                    "timeout".to_string(),
                    integer_prop_with_default(
                        Some(1),
                        Some(3600),
                        3600,
                        Some("Maximum seconds to wait when waitForResult is true. Ignored otherwise."),
                    ),
                ),
                ("task".to_string(), string_prop_required("The specific task description for the sub-agent.")),
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
        description: tool_description(
            "Mark the current root session as an explicit org root so its lineage appears in Org view.",
            &["Caller must be a top-level/root session."],
            &[
                "Provide a human-readable org name.",
                "If teamwork scaffold artifacts are missing, follow the tool result guidance.",
            ],
            &[
                "Prepare artifacts with agent__prepareTeamworkWorkspace if needed.",
                "Spawn org members with agent__startSession.",
            ],
        ),
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
        description: tool_description(
            "Get the current explicit org summary including root session and member sessions.",
            &[],
            &[
                "Omit orgId to use the caller session's org.",
                "Pass orgId when inspecting a specific org.",
            ],
            &[
                "Check member status with agent__checkSession.",
                "Message members with agent__messageToSession.",
            ],
        ),
        input_schema: object_prop(
            vec![(
                "orgId".to_string(),
                string_prop(
                    None,
                    None,
                    Some("Optional explicit org ID. If omitted, uses the caller session's org."),
                ),
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
        description: tool_description(
            "Send a follow-up message to an existing sub-agent session to continue or recover the conversation.",
            &["Session ID from agent__startSession or agent__listAgents(type='sessions')."],
            &[
                "Pass sessionId and the message or instruction.",
                "Use to wake paused or error sessions and retry from the latest stable state.",
                "Set waitForResponse=false to send without blocking.",
                "Set reset=true to clear the target session's message history and planning/compaction state before injecting the new message (defaults to false).",
            ],
            &[
                "Check outcome with agent__checkSession(wait=true).",
                "Stop stuck sessions with agent__stopSession.",
            ],
        ),
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
                (
                    "waitForResponse".to_string(),
                    {
                        let mut schema = boolean_prop(Some("If true (default), block until the child reaches a terminal response after receiving this message."));
                        schema.default = Some(json!(true));
                        schema
                    },
                ),
                (
                    "timeout".to_string(),
                    integer_prop_with_default(
                        Some(1),
                        Some(3600),
                        3600,
                        Some("Maximum seconds to wait when waitForResponse is true. Ignored otherwise."),
                    ),
                ),
                (
                    "reset".to_string(),
                    {
                        let mut schema = boolean_prop(Some("If true, clear/reset the target session's message history and planning/compaction state before injecting the new message (preserving active browser session state for continuation). Defaults to false."));
                        schema.default = Some(json!(false));
                        schema
                    },
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
        description: tool_description(
            "Check a sub-agent session status or wait for it to complete.",
            &["Session ID from agent__startSession or agent__listAgents(type='sessions')."],
            &[
                "Call with wait=false for a snapshot or wait=true to block until terminal state.",
                "After the child status/result, a fenced Metadata block adds identity/routing only (assistant, workspace) — not the child's answer. Session title/name is omitted.",
                "Paused or error sessions need recovery via agent__messageToSession.",
            ],
            &[
                "Recover paused sessions with agent__messageToSession.",
                "Terminate unnecessary sessions with agent__stopSession.",
            ],
        ),
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
        description: tool_description(
            "Forcefully terminate an active sub-agent session when a critical problem is confirmed.",
            &["Session ID from agent__checkSession or agent__listAgents(type='sessions')."],
            &[
                "CRITICAL: Do NOT stop a session simply because checkSession or messageToSession timed out. Timeouts only mean the child is busy working.",
                "Always check the child session's current status and latest messages via checkSession(wait=false) to confirm it is actually stuck before stopping.",
                "Use ONLY when there is a confirmed critical error, infinite loop, or the delegation is explicitly no longer needed.",
                "No-op if the session is already non-running.",
            ],
            &[
                "Check session status and messages first with agent__checkSession.",
                "Delete session data with agent__deleteSession if permanent removal is needed.",
            ],
        ),
        input_schema: object_prop(
            vec![(
                "sessionId".to_string(),
                string_prop_required("ID of the session to stop."),
            )],
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
        description: tool_description(
            "Force compaction for another delegated session and wait for the compact summary (cross-session only).",
            &["Target must be a delegated session — not the current session."],
            &[
                "Pass the child sessionId.",
                "Wait for compaction to finish (configurable timeout).",
            ],
            &[
                "Send more work with agent__messageToSession after compaction.",
                "Verify status with agent__checkSession.",
            ],
        ),
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

fn delete_session_tool() -> MCPTool {
    MCPTool {
        name: "deleteSession".to_string(),
        title: Some("Delete Agent Session".to_string()),
        description: tool_description(
            "Permanently delete a delegated descendant session and all its data.",
            &["Session must be a descendant of the current session — self-deletion is not allowed."],
            &[
                "Confirm the session is no longer needed.",
                "Pass the descendant sessionId.",
            ],
            &[
                "Stop running sessions first with agent__stopSession if needed.",
                "Verify org membership with agent__getOrg after deletion.",
            ],
        ),
        input_schema: object_prop(
            vec![
                ("sessionId".to_string(), string_prop_required("ID of the session to delete.")),
            ],
            vec!["sessionId".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}
