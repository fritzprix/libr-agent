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

❌ NEVER create without checking for duplicates first".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "name".to_string(),
                    string_prop_required("Assistant name (Must be unique)"),
                ),
                (
                    "systemPrompt".to_string(),
                    string_prop(None, None, Some("System prompt for the assistant")),
                ),
                (
                    "modelProvider".to_string(),
                    string_prop(None, None, Some("AI model provider (e.g., openai, anthropic, ollama)")),
                ),
                (
                    "modelName".to_string(),
                    string_prop(None, None, Some("Specific model name (e.g., gpt-4, claude-3-5-sonnet)")),
                ),
                (
                    "temperature".to_string(),
                    number_prop(Some(0.0), Some(1.0), Some("Model temperature (0.0 to 1.0)")),
                ),
                (
                    "maxTokens".to_string(),
                    integer_prop(None, None, Some("Maximum tokens for response")),
                ),
                (
                    "allowedBuiltInServiceAliases".to_string(),
                    array_schema(
                        string_prop(None, None, None),
                        Some("List of allowed built-in service aliases (e.g., 'mcp_manager', 'workspace', 'browser')"),
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
4. Update 'allowedBuiltInServiceAliases' to enable/disable builtin tools".to_string(),
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
                    "systemPrompt".to_string(),
                    string_prop(None, None, Some("New system prompt")),
                ),
                (
                    "modelProvider".to_string(),
                    string_prop(None, None, Some("New AI model provider")),
                ),
                (
                    "modelName".to_string(),
                    string_prop(None, None, Some("New model name")),
                ),
                (
                    "temperature".to_string(),
                    number_prop(None, None, Some("New temperature")),
                ),
                (
                    "maxTokens".to_string(),
                    integer_prop(None, None, Some("New max tokens")),
                ),
                (
                    "allowedBuiltInServiceAliases".to_string(),
                    array_schema(
                        string_prop(None, None, None),
                        Some("Update list of allowed built-in service aliases"),
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
