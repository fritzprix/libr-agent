use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// Save a knowledge entry to the assistant-scoped knowledge base
pub fn save_knowledge_tool() -> MCPTool {
    MCPTool {
        name: "saveKnowledge".to_string(),
        title: Some("Save Knowledge".to_string()),
        description: "Save a knowledge entry to the assistant-scoped knowledge base".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "title".to_string(),
                    string_prop_required("Title of the knowledge entry"),
                ),
                (
                    "content".to_string(),
                    string_prop_required("Content/body of the knowledge entry"),
                ),
                (
                    "source".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Source origin of the knowledge (e.g. URL, filename, 'user')"),
                    ),
                ),
                (
                    "tags".to_string(),
                    array_schema(
                        string_prop(None, None, None),
                        Some("Optional tags for categorization"),
                    ),
                ),
            ],
            vec!["title".to_string(), "content".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Read a specific knowledge entry by ID
pub fn read_knowledge_tool() -> MCPTool {
    MCPTool {
        name: "readKnowledge".to_string(),
        title: Some("Read Knowledge".to_string()),
        description: "Read a specific knowledge entry by ID".to_string(),
        input_schema: object_prop(
            vec![(
                "id".to_string(),
                integer_prop(None, None, Some("ID of the knowledge entry to read")),
            )],
            vec!["id".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Delete a specific knowledge entry by ID
pub fn delete_knowledge_tool() -> MCPTool {
    MCPTool {
        name: "deleteKnowledge".to_string(),
        title: Some("Delete Knowledge".to_string()),
        description: "Delete a specific knowledge entry by ID".to_string(),
        input_schema: object_prop(
            vec![(
                "id".to_string(),
                integer_prop(None, None, Some("ID of the knowledge entry to delete")),
            )],
            vec!["id".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Search the knowledge base using full-text search (FTS5) and/or tags
pub fn search_knowledge_tool() -> MCPTool {
    MCPTool {
        name: "searchKnowledge".to_string(),
        title: Some("Search Knowledge".to_string()),
        description: "Search the knowledge base using full-text search (FTS5) and/or tags"
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "query".to_string(),
                    string_prop(None, None, Some("Search query (FTS5 full-text search)")),
                ),
                (
                    "source".to_string(),
                    string_prop(None, None, Some("Filter by source")),
                ),
                (
                    "tags".to_string(),
                    array_schema(string_prop(None, None, None), Some("Filter by tags")),
                ),
                (
                    "limit".to_string(),
                    integer_prop_with_default(
                        None,
                        Some(100),
                        10,
                        Some("Maximum number of results"),
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

/// List all knowledge entries for this assistant (paginated)
pub fn list_knowledge_tool() -> MCPTool {
    MCPTool {
        name: "listKnowledge".to_string(),
        title: Some("List Knowledge".to_string()),
        description: "List all knowledge entries for this assistant (paginated)".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "limit".to_string(),
                    integer_prop_with_default(
                        Some(1),
                        Some(100),
                        20,
                        Some("Maximum number of entries"),
                    ),
                ),
                (
                    "offset".to_string(),
                    integer_prop_with_default(Some(0), None, 0, Some("Offset for pagination")),
                ),
            ],
            vec![],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Returns all knowledge tools
pub fn all_tools() -> Vec<MCPTool> {
    vec![
        save_knowledge_tool(),
        read_knowledge_tool(),
        delete_knowledge_tool(),
        search_knowledge_tool(),
        list_knowledge_tool(),
    ]
}
