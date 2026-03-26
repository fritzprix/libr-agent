use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// Record new knowledge into the local vector DB and graph.
pub fn record_knowledge_tool() -> MCPTool {
    MCPTool {
        name: "record_knowledge".to_string(),
        title: Some("Record Knowledge".to_string()),
        description: "Save a knowledge entry to the local knowledge base. The current implementation stores one chunk, generates an embedding, and can seed simple tag/entity links.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "content".to_string(),
                    string_prop_required("The full text content to store in the knowledge base."),
                ),
                (
                    "tags".to_string(),
                    array_schema(
                        string_prop(None, None, None),
                        Some("Optional tags for categorization (e.g. ['tech', 'project_alpha'])."),
                    ),
                ),
                (
                    "auto_extract".to_string(),
                    boolean_prop(
                        Some("Whether to derive simple entity links from the provided tags. Defaults to true. Full LLM-based extraction is not implemented yet."),
                    ),
                ),
                (
                    "source".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Optional source label for this knowledge entry (for example: conversation, file path, or URL)."),
                    ),
                ),
            ],
            vec!["content".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Search the knowledge base using a hybrid approach (Keyword + Semantic).
pub fn search_knowledge_tool() -> MCPTool {
    MCPTool {
        name: "search_knowledge".to_string(),
        title: Some("Search Knowledge".to_string()),
        description: "Search the knowledge base using keyword, semantic, or fused hybrid ranking.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "query".to_string(),
                    string_prop_required("The natural language question or keyword to search for."),
                ),
                (
                    "limit".to_string(),
                    integer_prop_with_default(
                        Some(1),
                        Some(50),
                        5,
                        Some("Maximum number of results to return (default: 5)."),
                    ),
                ),
                (
                    "mode".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Search mode: 'keyword', 'semantic', or 'hybrid'. Defaults to 'hybrid'."),
                    ),
                ),
            ],
            vec!["query".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Explore relationships around a central entity.
pub fn explore_context_tool() -> MCPTool {
    MCPTool {
        name: "explore_context".to_string(),
        title: Some("Explore Context".to_string()),
        description: "Explore the graph of relationships around a specific entity.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "entity_name".to_string(),
                    string_prop_required(
                        "The name of the central entity to explore (e.g., 'LibrAgent').",
                    ),
                ),
                (
                    "depth".to_string(),
                    integer_prop_with_default(
                        Some(1),
                        Some(3),
                        1,
                        Some("The depth of the graph traversal. Defaults to 1."),
                    ),
                ),
            ],
            vec!["entity_name".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Prune or manage existing knowledge.
pub fn prune_knowledge_tool() -> MCPTool {
    MCPTool {
        name: "prune_knowledge".to_string(),
        title: Some("Prune Knowledge".to_string()),
        description: "Delete or merge knowledge entries from the database.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "target_ids".to_string(),
                    array_schema(
                        integer_prop(None, None, None),
                        Some("List of knowledge chunk IDs to target."),
                    ),
                ),
                (
                    "action".to_string(),
                    string_prop_required(
                        "The action to perform: 'delete', 'update_importance', or 'merge'.",
                    ),
                ),
            ],
            vec!["target_ids".to_string(), "action".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Returns all knowledge tools
pub fn all_tools() -> Vec<MCPTool> {
    vec![
        record_knowledge_tool(),
        search_knowledge_tool(),
        explore_context_tool(),
        prune_knowledge_tool(),
    ]
}
