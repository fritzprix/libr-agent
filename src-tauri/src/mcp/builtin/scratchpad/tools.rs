use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

/// Get all scratchpad tools
pub fn all_tools() -> Vec<MCPTool> {
    vec![
        add_tool(),
        update_tool(),
        list_tool(),
        read_tool(),
        clear_tool(),
        think_tool(),
    ]
}

fn add_tool() -> MCPTool {
    MCPTool {
        name: "add".to_string(),
        title: Some("Add Scratchpad Note".to_string()),
        description: r#"Add a note to the Working Scratchpad. Content here is ALWAYS visible in your context. Use this for keeping track of important findings, file paths, IDs, or intermediate analysis results that you need to reference frequently during the task.

NOTE: Scratchpad has a strict limit of 10 items. If you reach this limit, use update to modify existing items or clear to remove old ones before adding more.
"#
        .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "content".to_string(),
                    string_prop_required(
                        r#"The content to add to scratchpad (e.g., "User requested feature X", "File path: src/main.ts")."#,
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
            vec!["content".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn update_tool() -> MCPTool {
    MCPTool {
        name: "update".to_string(),
        title: Some("Update Scratchpad Note".to_string()),
        description: "Update an existing scratchpad note. Use the ID shown in the scratchpad context or from list to identify which note to update.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "id".to_string(),
                    integer_prop(
                        None,
                        None,
                        Some("The ID of the scratchpad note to update (get from list or context)."),
                    ),
                ),
                (
                    "content".to_string(),
                    string_prop_required("The new content for the note."),
                ),
                (
                    "title".to_string(),
                    string_prop(None, None, Some("Optional: New title for the note.")),
                ),
            ],
            vec!["id".to_string(), "content".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn list_tool() -> MCPTool {
    MCPTool {
        name: "list".to_string(),
        title: Some("List Scratchpad Notes".to_string()),
        description: "List scratchpad notes with metadata (ID, title, tags) and content preview. Use this to find the IDs of items you want to read fully. Supports pagination and tag filtering.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "limit".to_string(),
                    integer_prop(Some(1), Some(100), Some("Maximum number of items to return (default: 10)")),
                ),
                (
                    "offset".to_string(),
                    integer_prop(Some(0), None, Some("Number of items to skip for pagination (default: 0)")),
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
        title: Some("Read Scratchpad Note".to_string()),
        description: "Read the FULL content of specific scratchpad notes by their IDs. Use list first to find IDs.".to_string(),
        input_schema: object_prop(
            vec![(
                "ids".to_string(),
                array_schema(
                    integer_prop(None, None, None),
                    Some("List of scratchpad note IDs to read (Required)."),
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
        title: Some("Clear Scratchpad Note".to_string()),
        description: "Remove a note from the Working Scratchpad. Use this to clear information that is no longer relevant to free up context window space.".to_string(),
        input_schema: object_prop(
            vec![(
                "id".to_string(),
                integer_prop(None, None, Some("The ID of the scratchpad note to remove.")),
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
