use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// List all registered MCP servers
pub fn list_servers_tool() -> MCPTool {
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
        description: "Search for MCP servers by name".to_string(),
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
    let transport_schema = object_prop(
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
        Some("Transport configuration"),
    );

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
• Use descriptive names for easy identification

IDENTIFICATION:
• System automatically generates a unique ID (UUID format)
• This ID is used in assistant configurations (mcpServerIds)
• Call listMcpServers after registration to get the auto-generated ID

RETURNS:
• Server name for management operations
• Auto-generated ID for assistant configuration
• Connection status

EXAMPLE:
  name: 'filesystem-workspace'
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
                        Some("Human-readable server name (e.g., 'filesystem-workspace', 'github-api'). Must be unique."),
                        vec![],
                    ),
                ),
                ("transport".to_string(), transport_schema),
                (
                    "description".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Optional description of the server's purpose and capabilities"),
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

/// Update configuration for an existing MCP server
pub fn update_server_tool() -> MCPTool {
    let transport_schema = object_prop(
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
        Some("New transport configuration"),
    );

    MCPTool {
        name: "updateServer".to_string(),
        title: Some("Update Server".to_string()),
        description: "Update configuration for an existing MCP server.

⚠️ PREREQUISITES:
1. Use listServers or searchServer to extract the target server 'name' (ID)
2. Server will restart automatically if currently running
3. For NPM packages: Use 'npx -y <package>' pattern

Returns:
• Update status
• Restart result if applicable
"
        .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "name".to_string(),
                    string_prop_required("Target name of the server to update"),
                ),
                ("transport".to_string(), transport_schema),
                (
                    "description".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Optional description of the server's purpose and capabilities"),
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

/// List all available internal LibrAgent tools (NEW NAME)
pub fn list_internal_tools_tool() -> MCPTool {
    MCPTool {
        name: "listInternalTools".to_string(),
        title: Some("List Internal Tools".to_string()),
        description: "List tool schemas from INTERNAL LibrAgent services.

⚠️ IMPORTANT: This tool does NOT work with external MCP servers.

🏠 INTERNAL SERVICES (this tool):
• planning, knowledge, browser, workspace
• content_store, assistant, playbook
• bootstrap, ui, mcp_manager

🌐 EXTERNAL SERVERS (not supported):
• Use listExternalServers to see user-registered servers
• Use verifyExternalServer to check specific server tools
• Add to assistant via updateAssistant(mcpServerIds: [...])

USAGE:
• No parameter: List all internal tools (88+ tools)
• serverName='workspace': Filter by specific internal service

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

/// List all available built-in MCP tools (DEPRECATED - use listInternalTools)
#[deprecated(since = "0.4.0", note = "Use list_internal_tools_tool() instead")]
pub fn list_builtin_tools_tool() -> MCPTool {
    let mut tool = list_internal_tools_tool();
    tool.name = "listBuiltinTools".to_string();
    tool.title = Some("List Builtin Tools (DEPRECATED)".to_string());
    tool.description = format!(
        "⚠️ DEPRECATED: Use 'listInternalTools' instead. This tool will be removed in v0.6.0.\n\n{}",
        tool.description
    );
    tool
}

/// List all registered external MCP servers (NEW NAME)
pub fn list_external_servers_tool() -> MCPTool {
    let mut tool = list_servers_tool();
    tool.name = "listExternalServers".to_string();
    tool.title = Some("List External Servers".to_string());
    tool.description = "List all registered EXTERNAL MCP servers.

🌐 EXTERNAL SERVERS (this tool):
• User-registered MCP servers (yahoo-finance-mcp, ddg-search, etc.)

🏠 INTERNAL SERVICES (use listInternalTools):
• LibrAgent built-in services (planning, knowledge, browser, workspace, etc.)

⚠️ MANDATORY:
1. Extract the 'name' from the list for subsequent operations.
2. Use this tool if server status is unknown.
"
    .to_string();
    tool
}

/// Returns all MCP Manager tools
pub fn all_tools() -> Vec<MCPTool> {
    vec![
        list_external_servers_tool(),
        list_internal_tools_tool(),
        search_server_tool(),
        register_server_tool(),
        update_server_tool(),
        delete_server_tool(),
        verify_server_tool(),
    ]
}
