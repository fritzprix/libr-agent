use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// Create a new global assistant configuration
pub fn create_assistant_tool() -> MCPTool {
    MCPTool {
        name: "createAssistant".to_string(),
        title: Some("Create Assistant".to_string()),
        description: "Create a new global assistant configuration.

⚠️ CRITICAL WORKFLOW (MUST FOLLOW):
1. ALWAYS call listAssistants FIRST to check for duplicates
2. Verify 'name' is unique
3. Then call this tool to create

❌ NEVER create without checking for duplicates first

🔒 RESTRICTION: You cannot modify your OWN assistant (the one running this
session). You can freely create and configure OTHER assistants, including
setting their 'allowedBuiltInServiceAliases'.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "name".to_string(),
                    string_prop_required("Assistant name (Must be unique)"),
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
                        Some("List of allowed built-in service aliases (e.g., 'workspace', 'browser', 'planning').\nControls which built-in tools this assistant can access."),
                    ),
                ),
                (
                    "mcpServerIds".to_string(),
                    array_schema(
                        string_prop(None, None, None),
                        Some("List of enabled MCP server IDs (UUIDs, NOT names).\n\n⚠️ CRITICAL: Use IDs, NOT server names!\n1. Call builtin_mcp_manager__listMcpServers FIRST\n2. Extract ID field (UUID format like 'cm3x...') from response\n3. NEVER use server name - it will fail validation\n4. Empty array = no external MCP servers\n\nExample valid ID: \"cm3xkn2w00000ld...\"\nExample INVALID: \"filesystem\" (this is a name, not ID)"),
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
        description: "Update an existing assistant configuration.

⚠️ CRITICAL WORKFLOW:
1. Call getAssistant(id) FIRST to get current config
2. Extract exact 'id' from response
3. Include ONLY fields you want to change

🔒 RESTRICTION: You cannot modify your OWN assistant (the one running this
session). You can freely update ANY OTHER assistant, including setting its
'allowedBuiltInServiceAliases'.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "id".to_string(),
                    string_prop_required("⚠️ Exact Assistant ID from getAssistant response"),
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
                        Some("Update the list of allowed built-in service aliases.\nControls which built-in tools this assistant can access."),
                    ),
                ),
                (
                    "mcpServerIds".to_string(),
                    array_schema(
                        string_prop(None, None, None),
                        Some("Update list of enabled MCP server IDs (UUIDs, NOT names).\n\n⚠️ CRITICAL: Use IDs, NOT server names!\n• Call builtin_mcp_manager__listMcpServers first\n• Extract ID field (UUID format) from response\n• NEVER use server name - validation will fail"),
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
        description: "Delete an assistant configuration.

⚠️ WARNING: This action is permanent.
✅ ALWAYS verify the ID with getAssistant before deleting"
            .to_string(),
        input_schema: object_prop(
            vec![(
                "id".to_string(),
                string_prop_required(
                    "⚠️ Exact Assistant ID from listAssistants/getAssistant response",
                ),
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
        description: "List available assistants with pagination.

Returns 'id', 'name', and 'config' for each assistant.
Use 'limit' and 'offset' to navigate through results."
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
                    string_prop(None, None, Some("Search term for filtering")),
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
        description: "Get full details of a specific assistant.

✅ Use this to retrieve the current configuration before updating."
            .to_string(),
        input_schema: object_prop(
            vec![(
                "id".to_string(),
                string_prop_required("⚠️ Exact Assistant ID from listAssistants response"),
            )],
            vec!["id".to_string()],
            None,
        ),
        annotations: None,
        output_schema: None,
    }
}

/// Search assistants by name or configuration content
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
        create_assistant_tool(),
        update_assistant_tool(),
        delete_assistant_tool(),
        list_assistants_tool(),
        get_assistant_tool(),
        search_assistant_tool(),
    ]
}
