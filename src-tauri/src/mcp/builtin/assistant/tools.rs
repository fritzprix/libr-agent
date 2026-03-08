use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// Create a new global assistant configuration
pub fn create_assistant_tool() -> MCPTool {
    MCPTool {
        name: "createAssistant".to_string(),
        title: Some("Create Assistant".to_string()),
        description: "Create a new global assistant configuration. \
Cannot modify your own running assistant. \
For mcpServerIds, use UUID IDs (from listMcpServers), NOT server names.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "name".to_string(),
                    string_prop_required("Unique assistant name"),
                ),
                (
                    "description".to_string(),
                    string_prop(None, None, Some("Short description shown on the assistant selection card (1-2 sentences)")),
                ),
                (
                    "systemPrompt".to_string(),
                    string_prop(None, None, Some("System prompt for the assistant")),
                ),
                (
                    "allowedBuiltInServiceAliases".to_string(),
                    array_schema(
                        string_prop(None, None, None),
                        Some("Built-in service aliases this assistant can access (e.g. 'workspace', 'browser', 'planning')"),
                    ),
                ),
                (
                    "mcpServerIds".to_string(),
                    array_schema(
                        string_prop(None, None, None),
                        Some("External MCP server UUIDs to enable. Call listMcpServers first to get IDs."),
                    ),
                ),
            ],
            vec!["name".to_string()],
            None,
        ),
        annotations: None,
        output_schema: None,
    }
}

/// Update an existing assistant configuration
pub fn update_assistant_tool() -> MCPTool {
    MCPTool {
        name: "updateAssistant".to_string(),
        title: Some("Update Assistant".to_string()),
        description: "Update an existing assistant configuration (partial update — omit fields to leave unchanged). \
Cannot modify your own running assistant.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "id".to_string(),
                    string_prop_required("Assistant ID from listAssistants/getAssistant"),
                ),
                (
                    "name".to_string(),
                    string_prop(None, None, Some("New name")),
                ),
                (
                    "description".to_string(),
                    string_prop(None, None, Some("Short description shown on the assistant selection card (1-2 sentences)")),
                ),
                (
                    "systemPrompt".to_string(),
                    string_prop(None, None, Some("New system prompt")),
                ),
                (
                    "allowedBuiltInServiceAliases".to_string(),
                    array_schema(
                        string_prop(None, None, None),
                        Some("Replaces the full list of allowed built-in service aliases"),
                    ),
                ),
                (
                    "mcpServerIds".to_string(),
                    array_schema(
                        string_prop(None, None, None),
                        Some("Replaces the full list of enabled MCP server UUIDs"),
                    ),
                ),
            ],
            vec!["id".to_string()],
            None,
        ),
        annotations: None,
        output_schema: None,
    }
}

/// Delete an assistant configuration
pub fn delete_assistant_tool() -> MCPTool {
    MCPTool {
        name: "deleteAssistant".to_string(),
        title: Some("Delete Assistant".to_string()),
        description: "Permanently delete an assistant configuration.".to_string(),
        input_schema: object_prop(
            vec![(
                "id".to_string(),
                string_prop_required("Assistant ID from listAssistants/getAssistant"),
            )],
            vec!["id".to_string()],
            None,
        ),
        annotations: None,
        output_schema: None,
    }
}

/// List available assistants with pagination
pub fn list_assistants_tool() -> MCPTool {
    MCPTool {
        name: "listAssistants".to_string(),
        title: Some("List Assistants".to_string()),
        description: "List available assistants. Use 'search' to filter by name or content."
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "limit".to_string(),
                    integer_prop_with_default(
                        None,
                        Some(100),
                        20,
                        Some("Items to return (max 100)"),
                    ),
                ),
                (
                    "offset".to_string(),
                    integer_prop_with_default(None, None, 0, Some("Items to skip")),
                ),
                (
                    "search".to_string(),
                    string_prop(None, None, Some("Filter by name or content")),
                ),
            ],
            vec![],
            None,
        ),
        annotations: None,
        output_schema: None,
    }
}

/// Get full details of a specific assistant
pub fn get_assistant_tool() -> MCPTool {
    MCPTool {
        name: "getAssistant".to_string(),
        title: Some("Get Assistant".to_string()),
        description: "Get full configuration of a specific assistant.".to_string(),
        input_schema: object_prop(
            vec![(
                "id".to_string(),
                string_prop_required("Assistant ID from listAssistants"),
            )],
            vec!["id".to_string()],
            None,
        ),
        annotations: None,
        output_schema: None,
    }
}

/// Search assistants by name or configuration content (internal — use listAssistants with search param instead)
pub fn search_assistant_tool() -> MCPTool {
    MCPTool {
        name: "searchAssistant".to_string(),
        title: Some("Search Assistant".to_string()),
        description: "Search assistants by name or configuration content.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "query".to_string(),
                    string_prop_required("Search query text"),
                ),
                (
                    "limit".to_string(),
                    integer_prop(None, None, Some("Maximum number of results")),
                ),
            ],
            vec!["query".to_string()],
            None,
        ),
        annotations: None,
        output_schema: None,
    }
}

/// Returns all assistant tools
pub fn all_tools() -> Vec<MCPTool> {
    vec![
        list_assistants_tool(),
        get_assistant_tool(),
        create_assistant_tool(),
        update_assistant_tool(),
        delete_assistant_tool(),
    ]
}
