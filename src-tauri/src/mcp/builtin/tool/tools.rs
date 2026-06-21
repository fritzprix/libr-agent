use crate::mcp::builtin::tool_description::tool_description;
use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

fn transport_config_schema(description: Option<&str>) -> crate::mcp::schema::JSONSchema {
    object_prop(
        vec![
            (
                "type".to_string(),
                enum_prop_required(vec!["stdio", "http", "http-sse"], "Transport type"),
            ),
            (
                "command".to_string(),
                string_prop(
                    None,
                    None,
                    Some("Executable to launch for stdio transport."),
                ),
            ),
            (
                "args".to_string(),
                array_schema(
                    string_prop(None, None, None),
                    Some("Arguments for the stdio executable."),
                ),
            ),
            (
                "env".to_string(),
                object_map_prop(Some("Environment variables for stdio transport.")),
            ),
            (
                "url".to_string(),
                string_prop(None, None, Some("Server URL for HTTP transport.")),
            ),
            (
                "protocolVersion".to_string(),
                string_prop(
                    None,
                    None,
                    Some(
                        "HTTP protocol version. If omitted, default: 2025-06-18.",
                    ),
                ),
            ),
            (
                "sessionId".to_string(),
                string_prop(
                    None,
                    None,
                    Some(
                        "Existing HTTP session ID to resume. If omitted, start a fresh HTTP session.",
                    ),
                ),
            ),
            (
                "headers".to_string(),
                object_map_prop(Some("HTTP headers for HTTP transport.")),
            ),
            (
                "enableSSE".to_string(),
                boolean_prop(Some(
                    "If true, use SSE for streaming HTTP transport. If omitted/false (default), use standard HTTP transport behavior.",
                )),
            ),
        ],
        vec!["type".to_string()],
        description,
    )
}

/// Register a new MCP server configuration
pub fn register_server_tool() -> MCPTool {
    let transport_schema = transport_config_schema(Some("Transport configuration"));

    MCPTool {
        name: "registerServer".to_string(),
        title: Some("Register Server".to_string()),
        description: tool_description(
            "Save an external MCP server configuration and return its server ID for assistant attachment.",
            &["Have transport details ready (stdio command or HTTP URL)."],
            &[
                "Choose a unique slug name (e.g., 'github', 'local-fs').",
                "Provide a short description of when agents should use this server.",
                "Configure transport (stdio, http, or http-sse) with required fields.",
            ],
            &[
                "Verify connectivity with tool__verifyServer.",
                "Attach the server ID to an agent via agent__updateAgent.",
                "Discover available tools with tool__listServers.",
            ],
        ),
        input_schema: object_prop(
            vec![
                (
                    "name".to_string(),
                    string_prop_with_examples(
                        Some(1),
                        Some(63),
                        Some("Unique human-readable slug for management (e.g., 'github', 'local-fs')."),
                        vec![],
                    ),
                ),
                (
                    "description".to_string(),
                    string_prop_required("Short purpose statement describing when this server should be used."),
                ),
                ("transport".to_string(), transport_schema),
            ],
            vec!["name".to_string(), "description".to_string(), "transport".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Update configuration for an existing MCP server
pub fn update_server_tool() -> MCPTool {
    let transport_schema = transport_config_schema(Some(
        "Replacement transport configuration. Set type to stdio (command, args, env) or http/http-sse (url, headers, enableSSE). Supply the full transport object for the target type.",
    ));

    MCPTool {
        name: "updateServer".to_string(),
        title: Some("Update Server".to_string()),
        description: tool_description(
            "Update a saved external MCP server configuration and re-verify it.",
            &["Know the server slug from tool__listServers."],
            &[
                "Provide the server name (slug) to update.",
                "Pass transport with the correct type and required fields for that transport.",
                "Optionally change description; omit fields you want to leave unchanged.",
            ],
            &[
                "Confirm the update with tool__verifyServer.",
                "Refresh agent tool lists if assistants reference this server.",
            ],
        ),
        input_schema: object_prop(
            vec![
                (
                    "name".to_string(),
                    string_prop_required("Target server name (slug) to update"),
                ),
                ("transport".to_string(), transport_schema),
                (
                    "description".to_string(),
                    string_prop(
                        None,
                        None,
                        Some(
                            "Optional new description. If omitted, keep the existing description unchanged.",
                        ),
                    ),
                ),
            ],
            vec!["name".to_string(), "transport".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Delete an MCP server configuration
pub fn delete_server_tool() -> MCPTool {
    MCPTool {
        name: "deleteServer".to_string(),
        title: Some("Delete Server".to_string()),
        description: tool_description(
            "Permanently delete a saved external MCP server configuration.",
            &["Confirm no active agents depend on this server (check agent__listAgents)."],
            &[
                "Identify the server slug from tool__listServers.",
                "Remove the configuration — this cannot be undone.",
            ],
            &[
                "Remove the server ID from agent configs via agent__updateAgent if needed.",
                "Register a replacement with tool__registerServer if required.",
            ],
        ),
        input_schema: object_prop(
            vec![(
                "name".to_string(),
                string_prop_required("Name of the server to delete"),
            )],
            vec!["name".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Verify server connectivity and configuration
pub fn verify_server_tool() -> MCPTool {
    MCPTool {
        name: "verifyServer".to_string(),
        title: Some("Verify Server".to_string()),
        description: tool_description(
            "Verify a saved external MCP server configuration and refresh its cached tools.",
            &["Server must already be registered via tool__registerServer."],
            &[
                "Pass the server slug name.",
                "Wait for connectivity check and tool cache refresh.",
            ],
            &[
                "Browse refreshed tools with tool__listServers (scope='external').",
                "Fix transport settings with tool__updateServer if verification fails.",
            ],
        ),
        input_schema: object_prop(
            vec![(
                "name".to_string(),
                string_prop_required("Server name to verify"),
            )],
            vec!["name".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Find tools across all sources in one call (unified discovery)
pub fn list_tools_tool() -> MCPTool {
    MCPTool {
        name: "listServers".to_string(),
        title: Some("List Servers".to_string()),
        description: tool_description(
            "Browse builtin tools and saved external MCP servers. Add query to filter results.",
            &[],
            &[
                "Choose scope (all, internal, external) and availability (inventory vs session).",
                "Use query to filter by tool name, description, or server name.",
                "Leave forceVerify=false (default) for fast cached external metadata; set true only when you need live re-verification.",
            ],
            &[
                "Register missing servers with tool__registerServer.",
                "Attach external server IDs to agents via agent__updateAgent.",
            ],
        ),
        input_schema: object_prop(
            vec![
                (
                    "query".to_string(),
                    string_prop(
                        None,
                        None,
                        Some(
                            "Filter over tool names, descriptions, and server names. If omitted, return unfiltered results.",
                        ),
                    ),
                ),
                (
                    "scope".to_string(),
                    enum_prop(
                        vec!["all", "internal", "external"],
                        "all",
                        Some(
                            "Result scope: all, builtin only, or external only. If omitted, default: all.",
                        ),
                    ),
                ),
                (
                    "availability".to_string(),
                    enum_prop(
                        vec!["inventory", "session"],
                        "inventory",
                        Some(
                            "inventory = list all registered servers and builtin tools. session = show only tools that are currently permitted in this active session.",
                        ),
                    ),
                ),
                (
                    "forceVerify".to_string(),
                    boolean_prop(Some(
                        "If true, live-verify each external server and refresh cached tool metadata (slower). If false or omitted (default), use metadata from the last tool__verifyServer run.",
                    )),
                ),
                (
                    "limit".to_string(),
                    integer_prop_with_default(
                        Some(1),
                        Some(100),
                        50,
                        Some("Maximum number of results to return (pagination). Default: 50."),
                    ),
                ),
                (
                    "offset".to_string(),
                    integer_prop_with_default(
                        Some(0),
                        None,
                        0,
                        Some("Number of results to skip (pagination). Default: 0."),
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

/// Returns all MCP Manager tools
pub fn all_tools() -> Vec<MCPTool> {
    vec![
        list_tools_tool(),
        register_server_tool(),
        update_server_tool(),
        delete_server_tool(),
        verify_server_tool(),
    ]
}
