use crate::mcp::builtin::tool_description::tool_description;
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
        name: "addNote".to_string(),
        title: Some("Add Scratchpad Note".to_string()),
        description: tool_description(
            "Add a note to the Working Scratchpad for this session only. Notes stay visible in your own context for findings, file paths, IDs, or intermediate results you reference often. Session-isolated: parent, child, and sibling sessions cannot read these notes — never hand off results by scratchpad ID alone; put deliverables in your final text response.",
            &["Scratchpad holds at most 10 items."],
            &[
                "If at the limit, update or clear existing notes first.",
                "Keep entries concise; use title and tags for scanability.",
            ],
            &[
                "Find note IDs with scratchpad__listNote.",
                "Update in place with scratchpad__updateNote instead of duplicating.",
            ],
        ),
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
        name: "updateNote".to_string(),
        title: Some("Update Scratchpad Note".to_string()),
        description: tool_description(
            "Update an existing scratchpad note by ID.",
            &["Note ID from scratchpad context or scratchpad__listNote."],
            &[
                "Identify the note ID to update.",
                "Replace content (and optionally title) with the latest information.",
            ],
            &[
                "Read full content with scratchpad__readNote if needed.",
                "Clear obsolete notes with scratchpad__clearNote.",
            ],
        ),
        input_schema: object_prop(
            vec![
                (
                    "id".to_string(),
                    integer_prop(
                        None,
                        None,
                        Some("The ID of the scratchpad note to update (get from listNote or context)."),
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
        name: "listNote".to_string(),
        title: Some("List Scratchpad Notes".to_string()),
        description: tool_description(
            "List scratchpad notes for this session only (ID, title, tags, content preview). Session-isolated: does not include notes from parent, child, or sibling sessions.",
            &[],
            &[
                "Use pagination when many notes exist.",
                "Filter by tags when looking for a category of notes.",
            ],
            &[
                "Read full content with scratchpad__readNote using returned IDs.",
                "Update notes with scratchpad__updateNote.",
            ],
        ),
        input_schema: object_prop(
            vec![
                (
                    "page".to_string(),
                    integer_prop(Some(1), None, Some("Page number (default: 1)")),
                ),
                (
                    "pageSize".to_string(),
                    integer_prop(Some(1), None, Some("Items per page (default: 10)")),
                ),
                (
                    "tags".to_string(),
                    array_schema(string_prop(None, None, None), Some("Filter items by tags")),
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
        name: "readNote".to_string(),
        title: Some("Read Scratchpad Note".to_string()),
        description: tool_description(
            "Read the full content of specific scratchpad notes by ID in this session only. Session-isolated: IDs from another session (including a sub-agent) are not readable here.",
            &["Note IDs from scratchpad__listNote or system context for the current session."],
            &[
                "Pass one or more IDs in the ids array.",
                "Use when previews from listNote are insufficient.",
            ],
            &[
                "Update content with scratchpad__updateNote.",
                "Remove stale notes with scratchpad__clearNote.",
            ],
        ),
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
        name: "clearNote".to_string(),
        title: Some("Clear Scratchpad Note".to_string()),
        description: tool_description(
            "Remove a note from the Working Scratchpad to free context window space.",
            &["Note ID from scratchpad__listNote."],
            &[
                "Confirm the information is no longer needed.",
                "Remove by ID — other notes remain.",
            ],
            &[
                "Add fresh notes with scratchpad__addNote when under the 10-item limit.",
                "List remaining notes with scratchpad__listNote.",
            ],
        ),
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
        description: tool_description(
            "Pause to reason through a problem before acting — evaluate options and plan the next move.",
            &[],
            &[
                "Write a concise reasoning summary in thought: the decision and next step only. Never dump full context, file contents, or long debate loops — a bloated thought wastes your output-token budget and can truncate the tool call.",
                "Optionally specify nextAction for what you will do immediately after.",
            ],
            &[
                "Execute the planned action with appropriate domain tools.",
                "Capture durable findings for this session with scratchpad__addNote (not a cross-session handoff).",
            ],
        ),
        input_schema: object_prop(
            vec![
                (
                    "thought".to_string(),
                    string_prop(
                        Some(1),
                        None,
                        Some("Concise reasoning summary (decision + next step). Summarize; do not paste full context or circular analysis."),
                    ),
                ),
                (
                    "nextAction".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Optional: What you plan to do next based on this thought."),
                    ),
                ),
            ],
            vec!["thought".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}
