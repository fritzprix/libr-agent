use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// Record new knowledge into the local vector DB and graph.
pub fn record_knowledge_tool() -> MCPTool {
    MCPTool {
        name: "record_knowledge".to_string(),
        title: Some("Record Knowledge".to_string()),
        description: "Save a knowledge entry to the local knowledge base. Prefer caller-supplied entities and relationships; heuristic extraction only fills gaps when structured graph data is missing.".to_string(),
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
                    "entities".to_string(),
                    array_schema(
                        object_prop(
                            vec![
                                (
                                    "name".to_string(),
                                    string_prop_required("Entity name as understood by the calling agent. Use a concise natural-language name with 10 words or fewer."),
                                ),
                                (
                                    "entity_type".to_string(),
                                    string_prop(
                                        None,
                                        None,
                                        Some("Optional entity type such as Project, Technology, Person, or Concept."),
                                    ),
                                ),
                                (
                                    "description".to_string(),
                                    string_prop(
                                        None,
                                        None,
                                        Some("Optional short description for the entity."),
                                    ),
                                ),
                            ],
                            vec!["name".to_string()],
                            Some("Structured entities inferred by the calling agent."),
                        ),
                        Some("Optional structured entities supplied by the caller."),
                    ),
                ),
                (
                    "relationships".to_string(),
                    array_schema(
                        object_prop(
                            vec![
                                (
                                    "source".to_string(),
                                    string_prop_required("Source entity name. Match entities[].name when the entity is supplied explicitly."),
                                ),
                                (
                                    "target".to_string(),
                                    string_prop_required("Target entity name. Match entities[].name when the entity is supplied explicitly."),
                                ),
                                (
                                    "relation_type".to_string(),
                                    string_prop_required("Relationship type such as USES, DEPENDS_ON, or LINKS_TO."),
                                ),
                            ],
                            vec![
                                "source".to_string(),
                                "target".to_string(),
                                "relation_type".to_string(),
                            ],
                            Some("Structured relationships inferred by the calling agent."),
                        ),
                        Some("Optional structured relationships supplied by the caller."),
                    ),
                ),
                (
                    "auto_extract".to_string(),
                    boolean_prop(
                        Some("Whether to run heuristic fallback extraction when structured entities or relationships are missing."),
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
        description: "Search the knowledge base using keyword, semantic, or fused hybrid ranking."
            .to_string(),
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
                        Some("Maximum number of results to return."),
                    ),
                ),
                (
                    "mode".to_string(),
                    enum_prop(
                        vec!["keyword", "semantic", "hybrid"],
                        "hybrid",
                        Some("'keyword' uses FTS, 'semantic' uses embeddings, and 'hybrid' fuses both rankings."),
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
        description: "Explore the graph of relationships around a specific entity and return agent-readable graph and linked chunk summaries.".to_string(),
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
                        Some("Number of relationship hops to traverse from the root entity."),
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
        description: "Delete knowledge entries from the database.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "target_ids".to_string(),
                    array_schema(
                        integer_prop(None, None, None),
                        Some(
                            "Knowledge chunk IDs to delete. Use IDs returned by search_knowledge.",
                        ),
                    ),
                ),
                (
                    "action".to_string(),
                    enum_prop_required(vec!["delete"], "Delete the targeted knowledge chunks."),
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
