use super::errors::{planning_follow_up_read_notice, planning_read_error, planning_write_error};
use crate::mcp::builtin::error_guidance::{
    guided_error, invalid_input_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::planning_repository::PlanningRepository;
use crate::state::get_planning_repository;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

/// Unified todo update — dispatches to check_todo or cancel_todo based on action.
pub async fn update_todo(
    db: &DatabaseConnection,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("done");

    match action {
        "done" => check_todo(db, session_id, args, true).await,
        "pending" => check_todo(db, session_id, args, false).await,
        "cancel" => cancel_todo(db, session_id, args).await,
        other => Ok(invalid_input_error(
            &format!(
                "Unknown action '{}'. Use 'done', 'pending', or 'cancel'.",
                other
            ),
            ToolGroup::Planning,
        )),
    }
}

/// Add a new todo - simplified version
///
/// HALLUCINATION FIREWALL: Strictly derives title from description and
/// enforces valid priority to prevent agents from creating malformed todos.
pub async fn add_todo(
    _db: &DatabaseConnection,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    // 1. Extract required parameters
    let description = match args.get("description").and_then(|v| v.as_str()) {
        Some(d) => d,
        None => return Ok(missing_param_error("description", ToolGroup::Planning)),
    };
    let description = description.trim();
    if description.is_empty() {
        return Ok(invalid_input_error(
            "Todo description cannot be blank",
            ToolGroup::Planning,
        ));
    }

    let priority = args
        .get("priority")
        .and_then(|v| v.as_str())
        .unwrap_or("medium");

    // 2. Validate priority
    if !["low", "medium", "high"].contains(&priority) {
        return Ok(invalid_input_error(
            "Invalid priority. Use: low, medium, or high",
            ToolGroup::Planning,
        ));
    }

    // 2b. Enforce description length limit to prevent context pollution
    const MAX_DESCRIPTION_CHARS: usize = 500;
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Ok(invalid_input_error(
            &format!(
                "Todo description too long ({} chars). Maximum is {} characters.",
                description.chars().count(),
                MAX_DESCRIPTION_CHARS
            ),
            ToolGroup::Planning,
        ));
    }

    // 3. Generate title from description (truncate to 50 chars)
    let title = if description.chars().count() > 50 {
        let truncated: String = description.chars().take(50).collect();
        format!("{}...", truncated)
    } else {
        description.to_string()
    };

    // 4. Insert todo (no parent, no duplicate check, no subtasks)
    let repo = get_planning_repository();
    match repo
        .add_todo(session_id, &title, Some(description.to_string()), priority)
        .await
    {
        Ok(id) => {
            let mut next_hints = vec![format!(
                "Use updateTodo(todoId={}, action='done') to mark as done",
                id
            )];
            let summary_text = match repo.get_planning_summary(session_id).await {
                Ok(summary_text) => summary_text,
                Err(error) => {
                    let notice = planning_follow_up_read_notice("updated planning summary", &error);
                    next_hints.push(notice.hint);
                    notice.suffix
                }
            };
            let hint = SuccessHint::new(
                format!("Added todo #{}: {}{}", id, title, summary_text),
                next_hints,
            );
            Ok(hint.to_mcp_result_with_data(Some(json!({
                "id": cuid2::create_id(),
                "success": true,
                "todoId": id,
                "todo": title
            }))))
        }
        Err(e) => Ok(planning_write_error(
            "add this todo",
            &e,
            vec![
                "Use getCurrentState to verify whether the todo was created.".to_string(),
                "Retry only if the todo is still missing.".to_string(),
            ],
        )),
    }
}

/// Check/Uncheck todo (also called by updateTodo action='done')
///
/// HALLUCINATION FIREWALL: Lists all todos to resolve the index-based position
/// before database access. Prevents agents from targeting non-existent todos via indices.
pub async fn check_todo(
    _db: &DatabaseConnection,
    session_id: &str,
    args: Value,
    checked: bool,
) -> Result<MCPResult, String> {
    // 1. Extract required parameters (todoId only)
    let todo_id = match args.get("todoId").and_then(|v| v.as_i64()) {
        Some(i) => i,
        None => return Ok(missing_param_error("todoId", ToolGroup::Planning)),
    };

    let summary = args
        .get("summary")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if todo_id <= 0 {
        return Ok(invalid_input_error(
            "todoId must be > 0",
            ToolGroup::Planning,
        ));
    }

    // 2. Fetch todo by ID and verify it belongs to the active session
    let repo = get_planning_repository();
    let todo = match repo.get_todo(todo_id).await {
        Ok(Some(todo)) if todo.session_id == session_id => todo,
        Ok(Some(_)) | Ok(None) => {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                format!("No todo found with todoId {}", todo_id),
                ToolGroup::Planning,
            )
            .with_guidance(vec![
                "Use getCurrentState to see current todos and their todo IDs".to_string(),
                "Copy the todoId exactly from the Planning service context or getCurrentState output"
                    .to_string(),
            ])
            .to_mcp_result());
        }
        Err(e) => {
            return Ok(planning_read_error(
                "read the requested todo",
                &e,
                vec![
                    "Use getCurrentState to refresh the current todo list.".to_string(),
                    "Retry after the planning store settles.".to_string(),
                ],
            ))
        }
    };
    let todo_content = todo.content.clone();

    // 3. Update (no parent auto-completion logic)
    if let Err(e) = repo.check_todo(todo_id, checked, summary).await {
        return Ok(planning_write_error(
            "update this todo",
            &e,
            vec![
                "Use getCurrentState to verify the final todo status before retrying.".to_string(),
                "Retry only if the status did not change.".to_string(),
            ],
        ));
    }

    let action = if checked { "completed" } else { "reopened" };
    let mut follow_up_hints = Vec::new();
    let summary_text = match repo.get_planning_summary(session_id).await {
        Ok(summary_text) => summary_text,
        Err(error) => {
            let notice = planning_follow_up_read_notice("updated planning summary", &error);
            follow_up_hints.push(notice.hint);
            notice.suffix
        }
    };

    // Check if all todos are now done (only when checking as done)
    let next_hints = if checked {
        match repo.list_todos(session_id, true).await {
            Ok(updated_todos) => {
                let remaining = updated_todos.iter().filter(|t| !t.is_checked).count();
                if remaining == 0 && !updated_todos.is_empty() {
                    vec![
                        "All todos complete! Use reflect to review what went well and what could improve."
                            .to_string(),
                    ]
                } else if remaining > 0 {
                    vec![format!(
                        "{} todo(s) remaining — use getCurrentState to see the list",
                        remaining
                    )]
                } else {
                    vec![]
                }
            }
            Err(error) => {
                let notice = planning_follow_up_read_notice("updated todo list", &error);
                follow_up_hints.push(notice.hint);
                vec![
                    "The todo status update succeeded, but the refreshed task list is unavailable right now."
                        .to_string(),
                ]
            }
        }
    } else {
        vec![format!(
            "Use updateTodo(todoId={}, action='done') to mark as done when completed",
            todo_id
        )]
    };
    let mut next_hints = next_hints;
    next_hints.extend(follow_up_hints);

    let hint = SuccessHint::new(
        format!(
            "Todo #{} marked {}: {}{}",
            todo_id, action, todo_content, summary_text
        ),
        next_hints,
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "id": cuid2::create_id(),
        "success": true,
        "todoId": todo_id,
        "checked": checked
    }))))
}

/// Cancel (permanently delete) a todo - simplified version
///
/// HALLUCINATION FIREWALL: Resolves index to ID via session-scoped list
/// to ensure agents only delete what is currently visible in their context.
pub async fn cancel_todo(
    _db: &DatabaseConnection,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    // 1. Extract required parameter (single todoId only)
    let todo_id = match args.get("todoId").and_then(|v| v.as_i64()) {
        Some(i) => i,
        None => return Ok(missing_param_error("todoId", ToolGroup::Planning)),
    };

    if todo_id <= 0 {
        return Ok(invalid_input_error(
            "todoId must be > 0",
            ToolGroup::Planning,
        ));
    }

    // 2. Fetch todo by ID and verify it belongs to the active session
    let repo = get_planning_repository();
    let todo = match repo.get_todo(todo_id).await {
        Ok(Some(todo)) if todo.session_id == session_id => todo,
        Ok(Some(_)) | Ok(None) => {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                format!("No todo found with todoId {}", todo_id),
                ToolGroup::Planning,
            )
            .with_guidance(vec![
                "Use getCurrentState to see current todos and their todo IDs".to_string(),
                "Copy the todoId exactly from the Planning service context or getCurrentState output"
                    .to_string(),
            ])
            .to_mcp_result());
        }
        Err(e) => {
            return Ok(planning_read_error(
                "read the requested todo",
                &e,
                vec![
                    "Use getCurrentState to refresh the current todo list.".to_string(),
                    "Retry after the planning store settles.".to_string(),
                ],
            ))
        }
    };
    let todo_content = todo.content.clone();

    // 3. Delete single todo (no batch, no "delete all")
    if let Err(e) = repo.delete_todos(session_id, vec![todo_id]).await {
        return Ok(planning_write_error(
            "remove this todo",
            &e,
            vec![
                "Use getCurrentState to verify whether the todo is still present.".to_string(),
                "Retry only if the todo was not removed.".to_string(),
            ],
        ));
    }

    let mut next_hints = vec![
        "Use addTodo to create a replacement if needed".to_string(),
        "Use getCurrentState to verify the updated task list".to_string(),
    ];
    let summary_text = match repo.get_planning_summary(session_id).await {
        Ok(summary_text) => summary_text,
        Err(error) => {
            let notice = planning_follow_up_read_notice("updated planning summary", &error);
            next_hints.push(notice.hint);
            notice.suffix
        }
    };

    let hint = SuccessHint::new(
        format!(
            "Removed todo #{}: {}{}",
            todo_id, todo_content, summary_text
        ),
        next_hints,
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "id": cuid2::create_id(),
        "success": true,
        "todoId": todo_id,
        "todo": todo_content
    }))))
}
