use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

fn transport_config_schema(description: Option<&str>) -> crate::mcp::schema::JSONSchema {
    object_prop(
        vec![
            (
                "type".to_string(),
                enum_prop_required(vec!["stdio", "http"], "Transport type"),
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
                object_prop(vec![], vec![], Some("Environment variables")),
            ),
            (
                "url".to_string(),
                string_prop(None, None, Some("Server URL (http only)")),
            ),
            (
                "headers".to_string(),
                object_prop(vec![], vec![], Some("HTTP headers")),
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
        description: "Register a new MCP server configuration.

⚠️ PREREQUISITES:
1. Verify target command exists before registration (stdio servers)
2. For NPM packages: Use 'npx -y <package>' (auto-installs on-demand)
3. For Python: Use 'uvx' or direct 'python -m' if installed
4. For Docker: Use 'docker run' with appropriate image

NAMING (REQUIRED):
• Provide human-readable 'name' (e.g., 'filesystem-workspace', 'github-api')
• Must be unique across all servers
• This 'name' is used for management operations (update/delete/verify)

IDENTIFICATION (SYSTEM):
• System automatically generates a unique ID (UUID format)
• This ID is required for assistant configurations (mcpServerIds)
• The ID is returned in the tool response upon successful registration

RETURNS:
• Server Name: For future management tool calls
• Server ID: Immutable UUID for assistant configurations
• Connection status

EXAMPLE:
  name: 'filesystem-workspace'
  description: 'Local filesystem access for reading and writing project files'
  transport:
    type: 'stdio'
    command: 'npx'
    args: ['-y', '@modelcontextprotocol/server-filesystem', '/workspace']
"
        .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "name".to_string(),
                    string_prop_with_examples(
                        Some(1),
                        Some(63),
                        Some("Human-readable unique name (slug) for identification. Used in update/delete/verify tool calls."),
                        vec![],
                    ),
                ),
                (
                    "description".to_string(),
                    string_prop_required("Detailed description of the server's purpose and capabilities. This helps the AI assistant understand when to use this server's tools."),
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
        name: "updateServer".to_string(),
        title: Some("Update Server".to_string()),
        description: "Update configuration for an existing MCP server. Use listTools to find the server name first.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "name".to_string(),
                    string_prop_required("Target name (slug) of the server to update"),
                ),
                ("transport".to_string(), transport_schema),
                (
                    "description".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Optional new description of the server's purpose"),
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
        description: "Delete an MCP server configuration. ⚠️ Permanent.".to_string(),
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
        description: "Test-connect an MCP server and cache its tool list. Use listTools to find the server name first.".to_string(),
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
        name: "listTools".to_string(),
        title: Some("Find Tools".to_string()),
        description: "Search tools across builtin services and external MCP servers.".to_string(),
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
