use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

/// Get all planning tools
pub fn all_tools() -> Vec<MCPTool> {
    vec![
        create_goal_tool(),
        update_goal_tool(),
        clear_goal_tool(),
        add_todo_tool(),
        update_todo_tool(),
        clear_session_tool(),
        get_current_state_tool(),
        reflect_tool(),
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

fn update_todo_tool() -> MCPTool {
    MCPTool {
        name: "updateTodo".to_string(),
        title: Some("Update Todo".to_string()),
        description: r#"Update a todo's status or cancel (remove) it, identified by its 0-based position.

action:
  'done'    — Mark as completed (stays in list for progress tracking).
  'pending' — Mark as incomplete (reopen a previously completed todo).
  'cancel'  — Permanently remove the todo. Use only when the task should never have existed.

Prefer 'done' over 'cancel' — completed todos preserve history.
Get positions from getCurrentState."#
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "index".to_string(),
                    integer_prop(
                        None,
                        Some(0),
                        Some("The 0-based position of the todo. Use getCurrentState to see current positions."),
                    ),
                ),
                (
                    "action".to_string(),
                    enum_prop(
                        vec!["done", "pending", "cancel"],
                        "done",
                        Some("The action to perform on the todo: 'done', 'pending', or 'cancel'."),
                    ),
                ),
                (
                    "summary".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Only for action='done'. Optional completion note (e.g., 'Fixed in PR #42')."),
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

fn reflect_tool() -> MCPTool {
    MCPTool {
        name: "reflect".to_string(),
        title: Some("Reflect".to_string()),
        description: "Critically reflect on progress after completing todos. Evaluate what went wrong or could be improved, then commit to a corrective next action.".to_string(),
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
