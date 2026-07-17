use crate::mcp::builtin::tool_description::tool_description;
use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;
use serde_json::json;

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
        description: tool_description(
            "Create a single outcome-focused goal for the session when starting a new or complex task.",
            &[],
            &[
                "Set one clear goal before adding todos.",
                "Describe the desired outcome, not individual steps.",
            ],
            &[
                "Break the goal into todos with planning__addTodo.",
                "Review progress with planning__getCurrentState.",
            ],
        ),
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
        description: tool_description(
            "Refine or correct the current session goal without clearing todos or other planning state.",
            &["An active goal should already exist (use planning__createGoal if none)."],
            &[
                "Confirm the goal still matches the user's intent.",
                "Replace the goal text with a clearer outcome statement.",
            ],
            &[
                "Align todos with planning__updateTodo if steps changed.",
                "Inspect full state with planning__getCurrentState.",
            ],
        ),
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
        description: tool_description(
            "Remove the current goal when the objective is complete or no longer relevant.",
            &[],
            &[
                "Confirm the goal is finished or abandoned.",
                "Clear only the goal; todos remain unless you clear them separately.",
            ],
            &[
                "Start a new objective with planning__createGoal.",
                "Reset everything with planning__clearSession if needed.",
            ],
        ),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

fn add_todo_tool() -> MCPTool {
    MCPTool {
        name: "addTodo".to_string(),
        title: Some("Add Todo".to_string()),
        description: tool_description(
            "Add a flat todo item to track a concrete step toward the session goal.",
            &[],
            &[
                "Ensure a goal exists or is implied before adding todos.",
                "Write one actionable step per todo (flat list only — no subtasks).",
            ],
            &[
                "Mark progress with planning__updateTodo (action='done').",
                "Review the list with planning__getCurrentState.",
            ],
        ),
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
        description: tool_description(
            "Mark a todo done/pending, or cancel it. Prefer done over cancel to keep history.",
            &[],
            &[],
            &[],
        ),
        input_schema: object_prop(
            vec![
                (
                    "id".to_string(),
                    integer_prop(Some(1), None, Some("Todo ID from Planning context (>= 1).")),
                ),
                (
                    "action".to_string(),
                    enum_prop(
                        vec!["done", "pending", "cancel"],
                        "done",
                        Some("Status action (default: done)."),
                    ),
                ),
                (
                    "summary".to_string(),
                    string_prop(None, None, Some("Optional note when action='done'.")),
                ),
            ],
            vec!["id".to_string()],
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
        description: tool_description(
            "Clear all session planning state (goal and todos) to start fresh.",
            &[],
            &[
                "Confirm the user wants to discard the current plan.",
                "This removes both goal and todos in one call.",
            ],
            &[
                "Set a new goal with planning__createGoal.",
                "Add fresh todos with planning__addTodo.",
            ],
        ),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

fn get_current_state_tool() -> MCPTool {
    MCPTool {
        name: "getCurrentState".to_string(),
        title: Some("Get Current State".to_string()),
        description: tool_description(
            "Get the current planning state (goal and todos) when you need IDs or details beyond system context.",
            &[],
            &[
                "Call when you need todo IDs before planning__updateTodo or planning__readNote-style lookups.",
                "Use include_checked=false to hide completed todos.",
            ],
            &[
                "Update todos with planning__updateTodo using returned IDs.",
                "Adjust the goal with planning__updateGoal if drifted.",
            ],
        ),
        input_schema: object_prop(
            vec![
                (
                    "include_checked".to_string(),
                    {
                        let mut schema = boolean_prop(Some(
                            "Whether to include checked todos in the output.",
                        ));
                        schema.default = Some(json!(true));
                        schema
                    },
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
        description: tool_description(
            "Record a structured self-critique after completing todos, then commit to a concrete corrective action.",
            &["Complete or review recent todos before reflecting."],
            &[
                "State what went wrong or could improve in critique.",
                "Capture what you learned in reflection.",
                "Define one concrete nextAction you will execute immediately.",
            ],
            &[
                "Add nextAction as a todo with planning__addTodo.",
                "Execute the corrective action before starting unrelated work.",
            ],
        ),
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
