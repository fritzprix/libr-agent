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
        name: "register".to_string(),
        title: Some("Register Server".to_string()),
        description:
            "Save an external MCP server configuration and return its server ID for assistant attachment."
                .to_string(),
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
    let transport_schema = transport_config_schema(Some("New transport configuration"));

    MCPTool {
        name: "update".to_string(),
        title: Some("Update Server".to_string()),
        description: "Update a saved external MCP server configuration and re-verify it."
            .to_string(),
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
        name: "delete".to_string(),
        title: Some("Delete Server".to_string()),
        description: "Delete a saved external MCP server configuration.".to_string(),
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
        name: "verify".to_string(),
        title: Some("Verify Server".to_string()),
        description:
            "Verify a saved external MCP server configuration and refresh its cached tools."
                .to_string(),
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
        name: "list".to_string(),
        title: Some("Find Tools".to_string()),
        description:
            "Browse builtin tools and saved external MCP servers. Add `query` to filter results."
                .to_string(),
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
                        "If true (default: false), fetch live external server metadata. If omitted/false, use cached metadata from the last verification.",
                    )),
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
