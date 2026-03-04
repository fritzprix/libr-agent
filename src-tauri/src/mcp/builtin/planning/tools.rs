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
        get_current_state_tool(),
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

fn get_current_state_tool() -> MCPTool {
    MCPTool {
        name: "getCurrentState".to_string(),
        title: Some("Get Current State".to_string()),
        description: "Get current planning state including Goal and Todos as human-readable text. Use when you need detailed visibility into current planning state beyond what's shown in the system context.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "include_checked".to_string(),
                    boolean_prop(Some(
                        "Whether to include checked todos in the output. Defaults to true.",
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
