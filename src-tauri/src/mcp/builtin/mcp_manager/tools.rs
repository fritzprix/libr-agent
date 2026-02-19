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

/// List all registered MCP servers (Base tool)
pub fn list_servers_tool_base() -> MCPTool {
    MCPTool {
        name: "listServers".to_string(),
        title: Some("List Servers".to_string()),
        description: "List all registered MCP servers
                
⚠️ MANDATORY:
1. Extract the 'name' from the list for subsequent target operations.
2. Use this tool if server status is unknown.
"
        .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "page".to_string(),
                    integer_prop(Some(1), None, Some("Page number for pagination")),
                ),
                (
                    "pageSize".to_string(),
                    integer_prop(Some(1), Some(50), Some("Items per page (max 50)")),
                ),
                (
                    "filterByAssistant".to_string(),
                    boolean_prop(Some("Filter servers by assistant capability")),
                ),
                (
                    "includeInactive".to_string(),
                    boolean_prop(Some("Include inactive/disconnected servers")),
                ),
            ],
            vec![],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Search for MCP servers by name
pub fn search_server_tool() -> MCPTool {
    let weights_schema = object_prop(
        vec![
            ("name".to_string(), number_prop(None, None, None)),
            ("description".to_string(), number_prop(None, None, None)),
        ],
        vec![],
        Some("Target search weights for fields"),
    );

    MCPTool {
        name: "searchServer".to_string(),
        title: Some("Search Server".to_string()),
        description: "[DEPRECATED] Use `listServers` with `query` parameter instead.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "query".to_string(),
                    string_prop_required("Search target query"),
                ),
                (
                    "page".to_string(),
                    integer_prop(Some(1), None, Some("Page number for pagination")),
                ),
                (
                    "pageSize".to_string(),
                    integer_prop(Some(1), Some(50), Some("Items per page (max 50)")),
                ),
                (
                    "searchMode".to_string(),
                    enum_prop(
                        vec!["simple", "bm25"],
                        "simple",
                        Some("Search mode (simple or bm25)"),
                    ),
                ),
                ("weights".to_string(), weights_schema),
            ],
            vec!["query".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
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
        description: "Update configuration for an existing MCP server.

⚠️ PREREQUISITES:
1. Use listExternalServers to find the target server 'name'
2. Server will restart automatically if currently running

Returns:
• Update status
• Server ID
"
        .to_string(),
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
        description: "Delete an MCP server configuration.
                
⚠️ WARNING: This action is permanent.
"
        .to_string(),
        input_schema: object_prop(
            vec![(
                "name".to_string(),
                string_prop_required("Target name of the server to exclude from configuration"),
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
        description:
            "Verify that an MCP server configuration is correct and the server can be connected.
                
This tool tests the server by:
- Validating configuration exists
- Attempting to spawn/connect (stdio/http)
- Calling listTools to verify functionality
- Reporting diagnostics (transport type, tool count, latency, errors)
                
⚠️ MANDATORY:
1. Extract the 'name' from 'listServers' FIRST.
2. This creates a temporary test connection and does not affect active sessions.
"
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

/// List all available built-in LibrAgent tools (NEW NAME)
pub fn list_builtin_tools_tool() -> MCPTool {
    MCPTool {
        name: "listBuiltinTools".to_string(),
        title: Some("List Built-in Tools".to_string()),
        description: "List tool schemas from BUILT-IN LibrAgent capabilities.

⚠️ IMPORTANT: This tool lists NATIVE capabilities (workspace, browser, etc.).

🏠 BUILT-IN SERVICES (this tool):
• planning, knowledge, browser, workspace
• content_store, assistant, playbook
• bootstrap, ui, mcp_manager

🌐 MCP SERVERS (use listServers):
• Use listServers to see user-added MCP servers
• Add to assistant via updateAssistant(mcpServerIds: [...])
• Add to assistant via updateAssistant(mcpServerIds: [...])

PAGINATION:
Results are paginated (20 tools per page) for large result sets.
"
        .to_string(),
        input_schema: object_prop(
            vec![(
                "serverName".to_string(),
                string_prop(
                    None,
                    None,
                    Some("Optional: Internal service name to filter by. Valid: planning, knowledge, browser, workspace, content_store, assistant, playbook, bootstrap, ui, mcp_manager"),
                ),
            )],
            vec![],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// List all registered MCP servers (NEW NAME)
pub fn list_servers_tool() -> MCPTool {
    let mut tool = list_servers_tool_base();
    tool.name = "listServers".to_string();
    tool.title = Some("List MCP Servers".to_string());
    tool.description = "List or search registered MCP servers.

🌐 SERVERS (this tool):
• User-added MCP servers (github-mcp, postgres-mcp, etc.)
• Use `query` parameter to filter by name.

🏠 BUILT-IN TOOLS (use listBuiltinTools):
• LibrAgent native capabilities (planning, workspace, etc.)

⚠️ MANDATORY:
1. Extract the 'id' (UUID) from the list for assistant configuration.
2. Use this tool if server status is unknown.
"
    .to_string();

    // Add query parameter to schema
    if let crate::mcp::schema::JSONSchemaType::Object {
        properties,
        required,
        ..
    } = &mut tool.input_schema.schema_type
    {
        if let Some(props) = properties {
            props.insert(
                "query".to_string(),
                string_prop(
                    None,
                    None,
                    Some("Optional search query to filter servers by name"),
                ),
            );
        }
        // Remove 'query' from required if it was copied from search tool (it wasn't, but safe to ensure)
        if let Some(req) = required {
            req.retain(|k| k != "query");
        }
    }

    tool
}

/// Returns all MCP Manager tools
pub fn all_tools() -> Vec<MCPTool> {
    vec![
        list_servers_tool(),
        list_builtin_tools_tool(),
        // searchServer is hidden (deprecated)
        register_server_tool(),
        update_server_tool(),
        delete_server_tool(),
        verify_server_tool(),
    ]
}
