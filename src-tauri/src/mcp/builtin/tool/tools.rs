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
                string_prop(None, None, Some("Command to execute (stdio only). Use 'npx' for NPM packages, 'uvx' for Python, 'docker' for containers. NEVER 'npm' or 'pip' install commands.")),
            ),
            (
                "args".to_string(),
                array_schema(
                    string_prop(None, None, None),
                    Some("Command arguments (stdio only). For npx, start with '-y' flag: ['-y', '@modelcontextprotocol/server-*', ...args]"),
                ),
            ),
            (
                "env".to_string(),
                object_map_prop(Some("Environment variables (stdio only)")),
            ),
            (
                "url".to_string(),
                string_prop(None, None, Some("Server URL (http only)")),
            ),
            (
                "protocolVersion".to_string(),
                string_prop(None, None, Some("Protocol version (http only, e.g. '2025-06-18')")),
            ),
            (
                "sessionId".to_string(),
                string_prop(None, None, Some("Session ID (http only)")),
            ),
            (
                "headers".to_string(),
                object_map_prop(Some("HTTP headers (http only)")),
            ),
            (
                "enableSSE".to_string(),
                boolean_prop(Some("Enable SSE (http only)")),
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
        description: "Register a new MCP server configuration. \
For NPM packages use 'npx' with args ['-y', '<pkg>']; for Python use 'uvx'. \
Returns the server ID needed to assign this server to an assistant."
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
                    string_prop_required("Detailed purpose of this server to help the AI understand when to use its tools."),
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
        description:
            "Update an existing MCP server configuration. Triggers an automatic re-verification."
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
                    string_prop(None, None, Some("Optional new description")),
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
        description: "Delete an MCP server configuration permanently.".to_string(),
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
        description: "Manually test an MCP server's connection and refresh its tool cache. Useful if the external service was down or changed.".to_string(),
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
        description: "Search servers and tools across builtin and external MCP servers. Omit query to see a compact summary of all servers.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "query".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Search term (tool name, description, server name). Empty = list all."),
                    ),
                ),
                (
                    "scope".to_string(),
                    enum_prop(
                        vec!["all", "internal", "external"],
                        "all",
                        Some("'all' (default), 'internal' (builtin only), 'external' (registered servers only)"),
                    ),
                ),
                (
                    "forceVerify".to_string(),
                    boolean_prop(Some("Connect live to external servers. Default: false (uses cached).")),
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
