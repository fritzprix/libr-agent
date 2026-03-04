use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

/// Get all memory tools
pub fn all_tools() -> Vec<MCPTool> {
    vec![
        add_tool(),
        update_tool(),
        list_tool(),
        read_tool(),
        clear_tool(),
        think_tool(),
        reflect_tool(),
    ]
}

fn add_tool() -> MCPTool {
    MCPTool {
        name: "add".to_string(),
        title: Some("Add Memory".to_string()),
        description: r#"Add a note to Working Memory. Content here is ALWAYS visible in your context. Use this for keeping track of important findings, file paths, IDs, or intermediate analysis results that you need to reference frequently during the task.

NOTE: Memory has a strict limit of 10 items. If you reach this limit, use update to modify existing items or clear to remove old ones before adding more.
"#
        .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "note".to_string(),
                    string_prop_required(
                        r#"The content to add to memory (e.g., "User requested feature X", "File path: src/main.ts")."#,
                    ),
                ),
                (
                    "title".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Optional title for the note. Helps in identifying the note in the list."),
                    ),
                ),
                (
                    "source".to_string(),
                    string_prop(
                        None,
                        None,
                        Some(r#"Optional source of the information for citation tracking. Examples: "https://example.com/article", "file://workspace/README.md", "tool_result_id:abc123""#),
                    ),
                ),
                (
                    "tags".to_string(),
                    array_schema(
                        string_prop(None, None, None),
                        Some("Optional tags for categorization and filtering."),
                    ),
                ),
            ],
            vec!["note".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn update_tool() -> MCPTool {
    MCPTool {
        name: "update".to_string(),
        title: Some("Update Memory".to_string()),
        description: "Update an existing memory note. Use the ID shown in the memory context or from list to identify which note to update.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "id".to_string(),
                    integer_prop(
                        None,
                        None,
                        Some("The ID of the memory note to update (get from list or context)."),
                    ),
                ),
                (
                    "note".to_string(),
                    string_prop_required("The new content for the note."),
                ),
                (
                    "title".to_string(),
                    string_prop(None, None, Some("Optional: New title for the note.")),
                ),
            ],
            vec!["id".to_string(), "note".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn list_tool() -> MCPTool {
    MCPTool {
        name: "list".to_string(),
        title: Some("List Memory".to_string()),
        description: "List memory notes with metadata (ID, title, tags) and content preview. Use this to find the IDs of items you want to read fully. Supports pagination and tag filtering.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "page".to_string(),
                    integer_prop(Some(1), Some(1), Some("Page number (default: 1)")),
                ),
                (
                    "pageSize".to_string(),
                    integer_prop(Some(10), Some(1), Some("Items per page (default: 10)")),
                ),
                (
                    "tags".to_string(),
                    array_schema(
                        string_prop(None, None, None),
                        Some("Filter items by tags"),
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

fn read_tool() -> MCPTool {
    MCPTool {
        name: "read".to_string(),
        title: Some("Read Memory".to_string()),
        description: "Read the FULL content of specific memory notes by their IDs. Use list first to find IDs.".to_string(),
        input_schema: object_prop(
            vec![(
                "ids".to_string(),
                array_schema(
                    integer_prop(None, Some(0), None),
                    Some("List of memory note IDs to read (Required)."),
                ),
            )],
            vec!["ids".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn clear_tool() -> MCPTool {
    MCPTool {
        name: "clear".to_string(),
        title: Some("Clear Memory".to_string()),
        description: "Remove a note from Working Memory. Use this to clear information that is no longer relevant to free up context window space.".to_string(),
        input_schema: object_prop(
            vec![(
                "id".to_string(),
                integer_prop(None, Some(0), Some("The ID of the memory note to remove.")),
            )],
            vec!["id".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn think_tool() -> MCPTool {
    MCPTool {
        name: "think".to_string(),
        title: Some("Think".to_string()),
        description: "Pause to reason through a problem before acting. Use this to process complex situations, evaluate options, or plan your next move. Helps avoid hasty decisions.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "thought".to_string(),
                    string_prop_required("Your reasoning, analysis, or chain of thought."),
                ),
                (
                    "nextAction".to_string(),
                    string_prop(None, None, Some("Optional: What you plan to do next based on this thought.")),
                ),
            ],
            vec!["thought".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn reflect_tool() -> MCPTool {
    MCPTool {
        name: "reflect".to_string(),
        title: Some("Reflect".to_string()),
        description: "Critically reflect on your progress. Evaluate what went wrong or could be improved, then commit to a corrective next action. Use when you detect a mistake, loop, or suboptimal approach.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "critique".to_string(),
                    string_prop_required("What went wrong or could be improved."),
                ),
                (
                    "reflection".to_string(),
                    string_prop_required("What you learned and how you will approach this differently."),
                ),
                (
                    "nextAction".to_string(),
                    string_prop_required("Concrete next action based on this reflection."),
                ),
            ],
            vec![
                "critique".to_string(),
                "reflection".to_string(),
                "nextAction".to_string(),
            ],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}
