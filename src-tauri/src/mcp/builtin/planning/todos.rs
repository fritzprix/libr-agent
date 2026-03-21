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
            // Find index of the newly created todo
            let all_todos = repo.list_todos(session_id, true).await.unwrap_or_default();
            let index = all_todos
                .iter()
                .position(|t| t.id == id)
                .unwrap_or(all_todos.len().saturating_sub(1));

            let summary_text = repo
                .get_planning_summary(session_id)
                .await
                .unwrap_or_default();
            let hint = SuccessHint::new(
                format!(
                    "Added todo #{} (index {}): {}{}",
                    id, index, title, summary_text
                ),
                vec![format!(
                    "Use updateTodo(index={}, action='done') to mark as done",
                    index
                )],
            );
            Ok(hint.to_mcp_result_with_data(Some(json!({
                "id": cuid2::create_id(),
                "success": true,
                "todoId": id,
                "index": index,
                "todo": title
            }))))
        }
        Err(e) => Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to add todo: {}", e),
            ToolGroup::Planning,
        )
        .with_guidance(vec![
            "Try again - this may be a transient database error".to_string(),
            "Use getCurrentState to verify if the todo was created".to_string(),
        ])
        .to_mcp_result()),
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
    // 1. Extract required parameters (index only)
    let index = match args.get("index").and_then(|v| v.as_i64()) {
        Some(i) => i,
        None => return Ok(missing_param_error("index", ToolGroup::Planning)),
    };

    let summary = args
        .get("summary")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if index < 0 {
        return Ok(invalid_input_error(
            "Index must be >= 0",
            ToolGroup::Planning,
        ));
    }

    // 2. Fetch todos to resolve index
    let repo = get_planning_repository();
    let todos = match repo.list_todos(session_id, true).await {
        Ok(t) => t,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::DatabaseError,
                format!("Failed to fetch todos: {}", e),
                ToolGroup::Planning,
            )
            .with_guidance(vec!["Try again".to_string()])
            .to_mcp_result())
        }
    };

    // 3. Get todo by index
    let todo = match todos.get(index as usize) {
        Some(t) => t,
        None => {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                format!("No todo found at position {}", index),
                ToolGroup::Planning,
            )
            .with_guidance(vec![
                "Use getCurrentState to see current todos and their positions".to_string(),
                "The index must be within range (0 to count-1)".to_string(),
            ])
            .to_mcp_result());
        }
    };

    let todo_id = todo.id;
    let todo_content = todo.content.clone();

    // 4. Update (no parent auto-completion logic)
    if let Err(e) = repo.check_todo(todo_id, checked, summary).await {
        return Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to update todo: {}", e),
            ToolGroup::Planning,
        )
        .with_guidance(vec![
            "Try again".to_string(),
            "Use getCurrentState to verify the final status".to_string(),
        ])
        .to_mcp_result());
    }

    let action = if checked { "completed" } else { "reopened" };
    let summary_text = repo
        .get_planning_summary(session_id)
        .await
        .unwrap_or_default();

    // Check if all todos are now done (only when checking as done)
    let next_hints = if checked {
        let updated_todos = repo.list_todos(session_id, true).await.unwrap_or_default();
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
    } else {
        vec!["Use updateTodo(index=N, action='done') to mark as done when completed".to_string()]
    };

    let hint = SuccessHint::new(
        format!(
            "Todo #{} (position {}) marked {}: {}{}",
            todo_id, index, action, todo_content, summary_text
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
    // 1. Extract required parameter (single index only)
    let index = match args.get("index").and_then(|v| v.as_i64()) {
        Some(i) => i,
        None => return Ok(missing_param_error("index", ToolGroup::Planning)),
    };

    if index < 0 {
        return Ok(invalid_input_error(
            "Index must be >= 0",
            ToolGroup::Planning,
        ));
    }

    // 2. Fetch todos to resolve index
    let repo = get_planning_repository();
    let todos = match repo.list_todos(session_id, true).await {
        Ok(t) => t,
        Err(e) => {
            return Ok(guided_error(
                ErrorCategory::DatabaseError,
                format!("Failed to fetch todos: {}", e),
                ToolGroup::Planning,
            )
            .with_guidance(vec!["Try again".to_string()])
            .to_mcp_result())
        }
    };

    // 3. Get todo by index
    let todo = match todos.get(index as usize) {
        Some(t) => t,
        None => {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                format!("No todo found at position {}", index),
                ToolGroup::Planning,
            )
            .with_guidance(vec![
                "Use getCurrentState to see current todos and their positions".to_string(),
                "The index must be within range (0 to count-1)".to_string(),
            ])
            .to_mcp_result());
        }
    };

    let todo_id = todo.id;
    let todo_content = todo.content.clone();

    // 4. Delete single todo (no batch, no "delete all")
    if let Err(e) = repo.delete_todos(session_id, vec![todo_id]).await {
        return Ok(guided_error(
            ErrorCategory::DatabaseError,
            format!("Failed to delete todo: {}", e),
            ToolGroup::Planning,
        )
        .with_guidance(vec![
            "Try again".to_string(),
            "Use getCurrentState to verify if it was removed".to_string(),
        ])
        .to_mcp_result());
    }

    let summary_text = repo
        .get_planning_summary(session_id)
        .await
        .unwrap_or_default();

    let hint = SuccessHint::new(
        format!(
            "Removed todo #{} (position {}): {}{}",
            todo_id, index, todo_content, summary_text
        ),
        vec![],
    );

    Ok(hint.to_mcp_result_with_data(Some(json!({
        "id": cuid2::create_id(),
        "success": true,
        "todoId": todo_id,
        "index": index,
        "todo": todo_content
    }))))
}
