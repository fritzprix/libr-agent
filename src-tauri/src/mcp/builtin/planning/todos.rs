use crate::mcp::builtin::error_guidance::{
    duplicate_error, invalid_input_error, missing_param_error, not_found_error, ErrorCategory,
    ErrorGuidance, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::PlanningRepository;
use crate::state::get_planning_repository;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

/// Add a new todo (Legacy: addTodo)
pub async fn add_todo(
    _db: &DatabaseConnection,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let description = args.get("description").and_then(|v| v.as_str());

    // Title is no longer part of the API schema.
    // We strictly derive it from the description.
    let title = if let Some(desc) = description {
        let trimmed = desc.trim();
        if !trimmed.is_empty() {
            // Truncate description to 50 chars for title
            if trimmed.chars().count() > 50 {
                let s: String = trimmed.chars().take(50).collect();
                format!("{}...", s)
            } else {
                trimmed.to_string()
            }
        } else {
            "Untitled Task".to_string()
        }
    } else {
        "Untitled Task".to_string()
    };
    let priority = args
        .get("priority")
        .and_then(|v| v.as_str())
        .unwrap_or("medium");
    let parent_id = args
        .get("parentId")
        .and_then(|v| v.as_i64())
        .filter(|&id| id > 0);

    // 1. Validate priority
    let valid_priorities = ["low", "medium", "high"];
    if !valid_priorities.contains(&priority) {
        return Ok(ErrorGuidance::with_guidance(
            ErrorCategory::InvalidInput,
            format!(
                "Invalid priority '{}'. Must be one of: low, medium, high",
                priority
            ),
            vec![
                "Use 'low', 'medium', or 'high' for priority".to_string(),
                format!(
                    "Example: {{\"priority\": \"high\"}} (you used: \"{}\")",
                    priority
                ),
                "Omit priority parameter to use default 'medium'".to_string(),
            ],
            ToolGroup::Planning,
        )
        .to_mcp_result());
    }

    // 2. Validate nesting constraints (cannot have both parentId and subtasks)
    if parent_id.is_some() && args.get("subtasks").is_some() {
        return Ok(ErrorGuidance::with_guidance(
            ErrorCategory::NestingTooDeep,
            "Cannot add subtasks to a child todo (max 1 level of nesting)",
            vec![
                "Create the todo without subtasks, then add subtasks separately".to_string(),
                "Create as top-level todo by omitting parentId".to_string(),
                "Use getCurrentState to see the current hierarchy".to_string(),
            ],
            ToolGroup::Planning,
        )
        .to_mcp_result());
    }

    let repo = get_planning_repository();

    // 3. Check for duplicate title (case-insensitive)
    match repo.check_todo_duplicate(session_id, &title).await {
        Ok(is_dup) => {
            if is_dup {
                return Ok(duplicate_error("Todo", &title, ToolGroup::Planning));
            }
        }
        Err(e) => {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::DatabaseError,
                format!("Failed to check duplicates: {}", e),
                vec!["Try again".to_string()],
                ToolGroup::Planning,
            )
            .to_mcp_result())
        }
    }

    // 4. If parent_id is provided, validate parent exists and is top-level
    if let Some(pid) = parent_id {
        match repo.get_todo(pid).await {
            Ok(Some(p)) => {
                // Ensure it belongs to this session (though get_todo generally fetches by ID, we should check session?)
                // The repository method get_todo relies on ID. IDs are unique globally or per session?
                // PlanningRepository `get_todo` returns `planning_todo::Model`.
                if p.session_id != session_id {
                    return Ok(not_found_error(
                        "Parent todo",
                        &pid.to_string(),
                        ToolGroup::Planning,
                    ));
                }

                if p.parent_id.is_some() {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::NestingTooDeep,
                        "Cannot add subtask to a subtask (max 1 level of nesting)",
                        vec![
                            "Create as top-level todo instead".to_string(),
                            "Attach to a different parent that has no parent".to_string(),
                            "Use getCurrentState to see the current hierarchy".to_string(),
                        ],
                        ToolGroup::Planning,
                    )
                    .to_mcp_result());
                }
            }
            Ok(None) => {
                return Ok(not_found_error(
                    "Parent todo",
                    &pid.to_string(),
                    ToolGroup::Planning,
                ));
            }
            Err(e) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::DatabaseError,
                    format!("Failed to fetch parent: {}", e),
                    vec!["Try again".to_string()],
                    ToolGroup::Planning,
                )
                .to_mcp_result())
            }
        }
    }

    // 5. Validate subtasks if present
    if let Some(subtasks) = args.get("subtasks").and_then(|v| v.as_array()) {
        for (idx, subtask) in subtasks.iter().enumerate() {
            // Relaxed subtask title validation
            let sub_desc = subtask.get("description").and_then(|v| v.as_str());
            let _sub_title = if let Some(desc) = sub_desc {
                let trimmed = desc.trim();
                if !trimmed.is_empty() {
                    if trimmed.chars().count() > 50 {
                        let s: String = trimmed.chars().take(50).collect();
                        format!("{}...", s)
                    } else {
                        trimmed.to_string()
                    }
                } else {
                    "Untitled Subtask".to_string()
                }
            } else {
                "Untitled Subtask".to_string()
            };

            // Validation: Ensure at least title or description is present
            if _sub_title == "Untitled Subtask" && sub_desc.is_none() {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Subtask #{} is missing both title and description.",
                        idx + 1
                    ),
                    vec![
                        "Provide a 'title' for the subtask".to_string(),
                        "OR provide a 'description' (title will be auto-generated)".to_string(),
                    ],
                    ToolGroup::Planning,
                )
                .to_mcp_result());
            }

            // Validate subtask priority
            let sub_prio = subtask
                .get("priority")
                .and_then(|v| v.as_str())
                .unwrap_or("medium");

            if !valid_priorities.contains(&sub_prio) {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Invalid subtask #{} priority '{}'. Must be one of: low, medium, high",
                        idx + 1,
                        sub_prio
                    ),
                    vec![
                        "Use 'low', 'medium', or 'high' for priority".to_string(),
                        "Omit priority to use default 'medium'".to_string(),
                    ],
                    ToolGroup::Planning,
                )
                .to_mcp_result());
            }
        }
    }

    match repo
        .add_todo(
            session_id,
            &title,
            description.map(|s| s.to_string()),
            priority,
            parent_id,
        )
        .await
    {
        Ok(id) => {
            // Handle subtasks if present
            if let Some(subtasks) = args.get("subtasks").and_then(|v| v.as_array()) {
                for subtask in subtasks {
                    let sub_desc = subtask.get("description").and_then(|v| v.as_str());

                    // Re-derive title
                    let sub_title = if let Some(desc) = sub_desc {
                        let trimmed = desc.trim();
                        if !trimmed.is_empty() {
                            if trimmed.chars().count() > 50 {
                                let s: String = trimmed.chars().take(50).collect();
                                format!("{}...", s)
                            } else {
                                trimmed.to_string()
                            }
                        } else {
                            "Untitled Subtask".to_string()
                        }
                    } else {
                        "Untitled Subtask".to_string()
                    };
                    let sub_prio = subtask
                        .get("priority")
                        .and_then(|v| v.as_str())
                        .unwrap_or("medium");

                    let _ = repo
                        .add_todo(
                            session_id,
                            &sub_title,
                            sub_desc.map(|s| s.to_string()),
                            sub_prio,
                            Some(id),
                        )
                        .await;
                }
            }

            let response_id = cuid2::create_id();
            let summary_text = repo
                .get_planning_summary(session_id)
                .await
                .unwrap_or_default();
            let hint = SuccessHint::new(
                format!("Todo added with ID {}: {}{}", id, title, summary_text),
                vec!["Use checkTodo when this task is done".to_string()],
            );
            Ok(hint.to_mcp_result_with_data(Some(json!({
                "id": response_id,
                "success": true,
                "todoId": id,
                "todo": title
            }))))
        }
        Err(e) => Ok(ErrorGuidance::with_guidance(
            ErrorCategory::DatabaseError,
            format!("Failed to add todo: {}", e),
            vec![
                "Try again - this may be a transient database error".to_string(),
                "Verify the session is active".to_string(),
                "Use getCurrentState to check if the todo was created despite the error"
                    .to_string(),
            ],
            ToolGroup::Planning,
        )
        .to_mcp_result()),
    }
}

/// Check/Uncheck todo (Legacy: checkTodo)
/// Check/Uncheck todo (Legacy: checkTodo)
pub async fn check_todo(
    _db: &DatabaseConnection,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let id = args.get("id").and_then(|v| v.as_i64());
    let index = args.get("index").and_then(|v| v.as_i64());
    let checked = args
        .get("checked")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let summary = args.get("summary").and_then(|v| v.as_str());

    let repo = get_planning_repository();

    let target_id = if let Some(tid) = id {
        if tid < 1 {
            return Ok(invalid_input_error(
                "Invalid 'id'. Must be >= 1",
                ToolGroup::Planning,
            ));
        }
        tid
    } else if let Some(idx) = index {
        if idx < 0 {
            return Ok(invalid_input_error(
                "Invalid 'index'. Must be >= 0",
                ToolGroup::Planning,
            ));
        }
        // Find ID by index (ordered by created_at)
        // We list all to find by index. This is less efficient than SQL LIMIT/OFFSET but we promised to move logic to repo/app.
        // Or we use existing repo.list_todos and pick.
        let all_todos = match repo.list_todos(session_id, true).await {
            Ok(todos) => todos,
            Err(e) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::DatabaseError,
                    format!("Failed to fetch todos: {}", e),
                    vec!["Try again".to_string()],
                    ToolGroup::Planning,
                )
                .to_mcp_result())
            }
        };

        if let Some(todo) = all_todos.get(idx as usize) {
            todo.id
        } else {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::ResourceNotFound,
                format!("Todo not found at index {}", idx),
                vec![
                    "Use getCurrentState to see available todos".to_string(),
                    format!("Index {} may be out of range", idx),
                    "Indices are 0-based and ordered by creation time".to_string(),
                ],
                ToolGroup::Planning,
            )
            .to_mcp_result());
        }
    } else {
        return Ok(missing_param_error("'id' or 'index'", ToolGroup::Planning));
    };

    // Check availability first
    let todo_item = match repo.get_todo(target_id).await {
        Ok(Some(t)) => {
            if t.session_id != session_id {
                return Ok(not_found_error(
                    "Todo",
                    &target_id.to_string(),
                    ToolGroup::Planning,
                ));
            }
            t
        }
        Ok(None) => {
            return Ok(not_found_error(
                "Todo",
                &target_id.to_string(),
                ToolGroup::Planning,
            ));
        }
        Err(e) => {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::DatabaseError,
                format!("Failed to fetch todo: {}", e),
                vec!["Try again".to_string()],
                ToolGroup::Planning,
            )
            .to_mcp_result())
        }
    };

    // Perform Check
    match repo
        .check_todo(target_id, checked, summary.map(|s| s.to_string()))
        .await
    {
        Ok(updated) => {
            if !updated {
                return Ok(not_found_error(
                    "Todo",
                    &target_id.to_string(),
                    ToolGroup::Planning,
                ));
            }

            // Auto-complete/reopen parent logic
            let mut parent_update_msg = String::new();
            if let Some(pid) = todo_item.parent_id {
                let should_check_parent = if !checked {
                    false
                } else {
                    // Check if all siblings are done
                    match repo.get_child_todos(pid).await {
                        Ok(children) => children.iter().all(|c| c.is_checked),
                        Err(_) => false,
                    }
                };

                // Check parent state
                if let Ok(Some(parent)) = repo.get_todo(pid).await {
                    if parent.is_checked != should_check_parent
                        && repo
                            .check_todo(pid, should_check_parent, None)
                            .await
                            .unwrap_or(false)
                    {
                        parent_update_msg = if should_check_parent {
                            " (Parent auto-completed)".to_string()
                        } else {
                            " (Parent auto-reopened)".to_string()
                        };
                    }
                }
            }

            let response_id = cuid2::create_id();
            let action = if checked { "checked" } else { "unchecked" };
            let summary_text = if let Some(s) = summary {
                format!(" - {}", s)
            } else {
                String::new()
            };

            let state_summary = repo
                .get_planning_summary(session_id)
                .await
                .unwrap_or_default();

            // Next actions
            // Use repo.list_todos(session_id, include_checked=false)
            let next_actions = match repo.list_todos(session_id, false).await {
                Ok(todos) => {
                    if todos.is_empty() {
                        vec!["All todos checked! Use 'critiqueAndReflection' to review work, or 'createGoal' to start a new objective.".to_string()]
                    } else {
                        let mut actions = Vec::new();
                        for todo in todos.iter().take(3) {
                            let content = &todo.content;
                            let safe_content = if content.chars().count() > 40 {
                                let truncated: String = content.chars().take(40).collect();
                                format!("{}...", truncated)
                            } else {
                                content.clone()
                            };
                            actions.push(format!(
                                "Process next: \"{}\" (ID: {})",
                                safe_content, todo.id
                            ));
                        }
                        actions
                    }
                }
                Err(_) => vec!["Could not fetch next actions".to_string()],
            };

            let hint = SuccessHint::new(
                format!(
                    "Todo {} (ID: {}){}{}{}",
                    action, target_id, summary_text, parent_update_msg, state_summary
                ),
                next_actions,
            );
            Ok(hint.to_mcp_result_with_data(Some(json!({
                "id": response_id,
                "success": true,
                "todoId": target_id,
                "checked": checked,
                "summary": summary
            }))))
        }
        Err(e) => Ok(ErrorGuidance::with_guidance(
            ErrorCategory::DatabaseError,
            format!("Failed to update todo: {}", e),
            vec![
                "Try again".to_string(),
                "Use getCurrentState to verify".to_string(),
            ],
            ToolGroup::Planning,
        )
        .to_mcp_result()),
    }
}

/// Cancel (permanently delete) todos (Legacy: clearTodos)
pub async fn cancel_todo(
    _db: &DatabaseConnection,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let ids = args.get("ids").and_then(|v| v.as_array());
    let indices = args.get("indices").and_then(|v| v.as_array());

    let repo = get_planning_repository();

    let mut target_ids: Vec<i64> = Vec::new();

    if ids.is_none() && indices.is_none() {
        // Cancel all: retrieve all IDs and delete
        match repo.list_todos(session_id, true).await {
            Ok(todos) => {
                target_ids = todos.into_iter().map(|t| t.id).collect();
            }
            Err(e) => {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::DatabaseError,
                    format!("Failed to list todos for cancellation: {}", e),
                    vec!["Try again".to_string()],
                    ToolGroup::Planning,
                )
                .to_mcp_result());
            }
        }
    } else {
        // Collect explicit IDs
        if let Some(id_list) = ids {
            for id_val in id_list {
                if let Some(id) = id_val.as_i64() {
                    if id < 1 {
                        return Ok(invalid_input_error(
                            "Invalid 'id'. Must be >= 1",
                            ToolGroup::Planning,
                        ));
                    }
                    target_ids.push(id);
                }
            }
        }

        // Collect IDs from indices
        if let Some(idx_list) = indices {
            let all_todos = match repo.list_todos(session_id, true).await {
                Ok(todos) => todos,
                Err(e) => {
                    return Ok(ErrorGuidance::with_guidance(
                        ErrorCategory::DatabaseError,
                        format!("Failed to list todos for index mapping: {}", e),
                        vec!["Try again".to_string()],
                        ToolGroup::Planning,
                    )
                    .to_mcp_result());
                }
            };

            for idx_val in idx_list {
                if let Some(idx) = idx_val.as_i64() {
                    if idx < 0 {
                        return Ok(invalid_input_error(
                            "Invalid 'index'. Must be >= 0",
                            ToolGroup::Planning,
                        ));
                    }
                    let idx = idx as usize;
                    if idx < all_todos.len() {
                        target_ids.push(all_todos[idx].id);
                    }
                }
            }
        }
    }

    if target_ids.is_empty() {
        return Ok(SuccessHint::new(
            "✓ No todos found to cancel".to_string(),
            vec!["Use 'getCurrentState' to see available todos".to_string()],
        )
        .to_mcp_result());
    }

    // Remove duplicates
    target_ids.sort();
    target_ids.dedup();

    match repo.delete_todos(session_id, target_ids).await {
        Ok(count) => {
            let summary_text = repo
                .get_planning_summary(session_id)
                .await
                .unwrap_or_default();

            // Generate next actions for hint
            let next_actions = match repo.list_todos(session_id, false).await {
                Ok(todos) => {
                    if todos.is_empty() {
                        vec![
                            "All todos cancelled! Use 'critiqueAndReflection' to review work."
                                .to_string(),
                            "Use 'createGoal' to start a new objective.".to_string(),
                        ]
                    } else {
                        let mut actions = Vec::new();
                        for todo in todos.iter().take(2) {
                            let content = &todo.content;
                            let safe_content = if content.chars().count() > 40 {
                                let truncated: String = content.chars().take(40).collect();
                                format!("{}...", truncated)
                            } else {
                                content.clone()
                            };
                            actions.push(format!(
                                "Process next: \"{}\" (ID: {})",
                                safe_content, todo.id
                            ));
                        }
                        actions.push(
                            "Use 'checkTodo' to mark done, or 'cancelTodo' to remove more."
                                .to_string(),
                        );
                        actions
                    }
                }
                Err(_) => vec![],
            };

            let hint = SuccessHint::new(
                format!("✓ Cancelled {} todos{}", count, summary_text),
                next_actions,
            );
            Ok(hint.to_mcp_result())
        }
        Err(e) => Ok(ErrorGuidance::with_guidance(
            ErrorCategory::DatabaseError,
            format!("Failed to cancel todos: {}", e),
            vec![
                "Try again".to_string(),
                "Use getCurrentState to verify".to_string(),
            ],
            ToolGroup::Planning,
        )
        .to_mcp_result()),
    }
}
