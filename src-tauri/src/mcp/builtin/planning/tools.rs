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
        description: "Clear the current goal when the objective is complete or no longer relevant."
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
        description: "Add a todo item. Flat structure only — no subtasks or nesting.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "description".to_string(),
                    string_prop(None, Some(500), Some("The task to be done.")),
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
        description: r#"Update a todo's status or permanently remove it.

action semantics:
  'done'    — Completes the todo; stays in list for history.
  'pending' — Reopens a previously completed todo.
  'cancel'  — Permanently removes it. Only when the task should never have existed; prefer 'done' to preserve history."#
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "todoId".to_string(),
                    integer_prop(
                        None,
                        None,
                        Some("The unique todo ID. Use getCurrentState to see current todo IDs."),
                    ),
                ),
                (
                    "action".to_string(),
                    enum_prop(
                        vec!["done", "pending", "cancel"],
                        "done",
                        Some("Action to apply (default: 'done')."),
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
            vec!["todoId".to_string()],
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
        description: "Clear all session planning state (goal and todos). Use to reset the plan and start fresh."
            .to_string(),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

fn get_current_state_tool() -> MCPTool {
    MCPTool {
        name: "getCurrentState".to_string(),
        title: Some("Get Current State".to_string()),
        description: "Get current planning state (goal and todos). Use when you need detailed visibility beyond what's shown in the system context.".to_string(),
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
        description: "Record a structured self-critique after completing todos, then commit to a concrete corrective action.".to_string(),
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
