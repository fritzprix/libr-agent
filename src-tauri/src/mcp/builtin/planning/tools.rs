use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

/// Get all planning tools
pub fn all_tools() -> Vec<MCPTool> {
    vec![
        create_goal_tool(),
        update_goal_tool(),
        clear_goal_tool(),
        add_todo_tool(),
        check_todo_tool(),
        cancel_todo_tool(),
        clear_session_tool(),
        add_scratchpad_tool(),
        update_scratchpad_tool(),
        list_scratchpad_tool(),
        read_scratchpad_tool(),
        clear_scratchpad_tool(),
        get_current_state_tool(),
        pause_and_think_tool(),
        critique_and_reflection_tool(),
    ]
}

fn create_goal_tool() -> MCPTool {
    MCPTool {
        name: "createGoal".to_string(),
        title: Some("Create Goal".to_string()),
        description:
            "Create a single goal for the session. Use when starting a new or complex task."
                .to_string(),
        input_schema: object_prop(
            vec![(
                "goal".to_string(),
                string_prop_required(
                    "The goal text to set for the session (e.g., \"Complete project setup\").",
                ),
            )],
            vec!["goal".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn update_goal_tool() -> MCPTool {
    MCPTool {
        name: "updateGoal".to_string(),
        title: Some("Update Goal".to_string()),
        description: "Update the current goal. Use when the goal needs refinement or correction without clearing context.".to_string(),
        input_schema: object_prop(
            vec![(
                "goal".to_string(),
                string_prop_required("The new goal text."),
            )],
            vec!["goal".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn clear_goal_tool() -> MCPTool {
    MCPTool {
        name: "clearGoal".to_string(),
        title: Some("Clear Goal".to_string()),
        description: "Clear the current goal. Use when finishing or abandoning the current goal."
            .to_string(),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

fn add_todo_tool() -> MCPTool {
    MCPTool {
        name: "addTodo".to_string(),
        title: Some("Add Todo".to_string()),
        description: "Add a simple todo item. No subtasks, no nesting - flat structure only. Use to track individual tasks.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "description".to_string(),
                    string_prop_required("The task to be done."),
                ),
                (
                    "priority".to_string(),
                    enum_prop(
                        vec!["low", "medium", "high"],
                        "medium",
                        Some("Priority level (default: medium)."),
                    ),
                ),
            ],
            vec!["description".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn check_todo_tool() -> MCPTool {
    MCPTool {
        name: "checkTodo".to_string(),
        title: Some("Check Todo".to_string()),
        description: "Mark a todo as done or undone using its position. Get positions from getCurrentState. Checked todos remain in the list for progress tracking.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "index".to_string(),
                    integer_prop(
                        None,
                        Some(0),
                        Some("The 0-based position of the todo in the list (e.g., 0 for first todo, 1 for second)"),
                    ),
                ),
                (
                    "checked".to_string(),
                    boolean_prop(Some("Whether to mark as done (true) or undone (false). Defaults to true.")),
                ),
                (
                    "summary".to_string(),
                    string_prop(None, None, Some("Optional completion summary (e.g., 'Fixed with PR #42', 'Resolved in commit abc123').")),
                ),
            ],
            vec!["index".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn cancel_todo_tool() -> MCPTool {
    MCPTool {
        name: "cancelTodo".to_string(),
        title: Some("Cancel Todo".to_string()),
        description: r#"Remove a single todo by its position. Use this tool when:
• Task was created incorrectly
• Requirements changed and task is no longer needed
• Task duplicates another todo

⚠️ IMPORTANT: This operation is irreversible
❌ DO NOT use for completed tasks - use checkTodo instead to preserve completion history
✓ Use cancelTodo only for tasks that should not exist"#.to_string(),
        input_schema: object_prop(
            vec![
                (
                    "index".to_string(),
                    integer_prop(
                        None,
                        Some(0),
                        Some("The 0-based position of the todo to remove (e.g., 0 for first todo, 1 for second)"),
                    ),
                ),
            ],
            vec!["index".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn clear_session_tool() -> MCPTool {
    MCPTool {
        name: "clearSession".to_string(),
        title: Some("Clear Session".to_string()),
        description: "Clear all session state (goal, todos, and scratchpad items). Use to reset everything and start fresh.".to_string(),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

fn add_scratchpad_tool() -> MCPTool {
    MCPTool {
        name: "addScratchpad".to_string(),
        title: Some("Add Scratchpad".to_string()),
        description: r#"Add a note to your Scratchpad (Working Memory). Content here is ALWAYS visible in your context. Use this for keeping track of important findings, file paths, IDs, or intermediate analysis results that you need to reference frequently during the task.

NOTE: The scratchpad has a strict limit of 10 items. If you reach this limit, you must use updateScratchpad to modify existing items or clearScratchpad to remove old ones before adding more.
"#.to_string(),
        input_schema: object_prop(
            vec![
                (
                    "note".to_string(),
                    string_prop_required(r#"The content to add to the scratchpad (e.g., "User requested feature X", "File path: src/main.ts")."#),
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
                        Some(r#"Optional source of the information for citation tracking. Examples: "https://example.com/article", "file://workspace/docs/readme.md", "tool_result_id:abc123""#),
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

fn update_scratchpad_tool() -> MCPTool {
    MCPTool {
        name: "updateScratchpad".to_string(),
        title: Some("Update Scratchpad".to_string()),
        description: "Update an existing scratchpad note. Use the ID shown in getCurrentState or listScratchpad to identify which note to update.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "id".to_string(),
                    integer_prop(None, None, Some("The ID of the scratchpad note to update (get from getCurrentState or listScratchpad).")),
                ),
                (
                    "note".to_string(),
                    string_prop_required("The new content for the note."),
                ),
                (
                    "title".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Optional: New title for the note."),
                    ),
                ),
            ],
            vec!["id".to_string(), "note".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn list_scratchpad_tool() -> MCPTool {
    MCPTool {
        name: "listScratchpad".to_string(),
        title: Some("List Scratchpad".to_string()),
        description: "List scratchpad items with metadata (ID, title, tags) and content preview. Use this to find the IDs of items you want to read fully. Supports pagination and tag filtering.".to_string(),
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

fn read_scratchpad_tool() -> MCPTool {
    MCPTool {
        name: "readScratchpad".to_string(),
        title: Some("Read Scratchpad".to_string()),
        description: "Read the FULL content of specific scratchpad items by their IDs. You must provide the IDs of the items you want to read. Use listScratchpad first to find IDs.".to_string(),
        input_schema: object_prop(
            vec![(
                "ids".to_string(),
                array_schema(
                    integer_prop(None, Some(0), None),
                    Some("List of scratchpad IDs to read (Required)."),
                ),
            )],
            vec!["ids".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn clear_scratchpad_tool() -> MCPTool {
    MCPTool {
        name: "clearScratchpad".to_string(),
        title: Some("Clear Scratchpad".to_string()),
        description: "Remove a note from your Scratchpad. Use this to clear information that is no longer relevant to free up context window space.".to_string(),
        input_schema: object_prop(
            vec![(
                "id".to_string(),
                integer_prop(
                    None,
                    Some(0),
                    Some("The ID of the scratchpad item to clear."),
                ),
            )],
            vec!["id".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn get_current_state_tool() -> MCPTool {
    MCPTool {
        name: "getCurrentState".to_string(),
        title: Some("Get Current State".to_string()),
        description: "Get current planning state including Goal, Todos, and Scratchpad as human-readable text. Use when you need detailed visibility into current planning state beyond what's shown in the system context.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "include_checked".to_string(),
                    boolean_prop(Some(
                        "Whether to include checked todos in the output. Defaults to true.",
                    )),
                ),
                (
                    "include_scratchpad".to_string(),
                    boolean_prop(Some(
                        "Whether to include scratchpad items in the output. Defaults to true.",
                    )),
                ),
            ],
            vec![],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn pause_and_think_tool() -> MCPTool {
    MCPTool {
        name: "pauseAndThink".to_string(),
        title: Some("Pause and Think".to_string()),
        description: "Pause to think about the problem, plan your approach, or analyze results before taking action. Use this when you need to reason through complex decisions or maintain context. Simpler alternative to sequentialthinking.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "thought".to_string(),
                    string_prop_required(
                        "Your current thought, analysis, or plan. Be clear and specific about what you are thinking through.",
                    ),
                ),
                (
                    "nextAction".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Optional: The specific next action you plan to take after this thought. Helps maintain continuity."),
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

fn critique_and_reflection_tool() -> MCPTool {
    MCPTool {
        name: "critiqueAndReflection".to_string(),
        title: Some("Critique and Reflection".to_string()),
        description: "Reflect on the current state and provide a critique of the progress. Use this tool to pause, analyze what has been done, identify potential issues or missed steps, and plan the next actions carefully.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "critique".to_string(),
                    string_prop_required(
                        "A critical evaluation of the results achieved so far.",
                    ),
                ),
                (
                    "reflection".to_string(),
                    string_prop_required(
                        "Self-reflection on any shortcomings or areas for improvement in the process.",
                    ),
                ),
                (
                    "nextAction".to_string(),
                    string_prop_required(
                        "The expected next action based on the reflection.",
                    ),
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
