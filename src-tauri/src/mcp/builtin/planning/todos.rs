use crate::mcp::builtin::error_guidance::{
    duplicate_error, invalid_input_error, missing_param_error, not_found_error, ErrorCategory,
    ErrorGuidance, SuccessHint, ToolGroup,
};
use crate::mcp::builtin::planning::context::get_planning_summary;
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use sqlx::SqlitePool;

/// Add a new todo (Legacy: addTodo)
pub async fn add_todo(
    pool: &SqlitePool,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    // Validate title parameter
    let title = match args
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        Some(t) => t,
        Option::None => {
            return Ok(ErrorGuidance::with_guidance(
                ErrorCategory::MissingRequiredParam,
                "Missing or empty 'title' parameter",
                vec![
                    "Provide a non-empty title string".to_string(),
                    "Example: {\"title\": \"Implement feature X\"}".to_string(),
                    "Use getCurrentState to see existing todos".to_string(),
                ],
                ToolGroup::Planning,
            )
            .to_mcp_result());
        }
    };

    let description = args.get("description").and_then(|v| v.as_str());
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

    // 3. Check for duplicate title (case-insensitive)
    let duplicate_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM planning_todos WHERE session_id = ? AND lower(content) = lower(?)",
    )
    .bind(session_id)
    .bind(title)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to check duplicates: {}", e))?;

    if duplicate_count > 0 {
        return Ok(duplicate_error("Todo", title, ToolGroup::Planning));
    }

    // 4. If parent_id is provided, validate parent exists and is top-level
    if let Some(pid) = parent_id {
        let parent: Option<(Option<i64>,)> =
            sqlx::query_as("SELECT parent_id FROM planning_todos WHERE id = ? AND session_id = ?")
                .bind(pid)
                .bind(session_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| format!("Failed to fetch parent: {}", e))?;

        match parent {
            Some((grandparent_id,)) => {
                if grandparent_id.is_some() {
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
            Option::None => {
                return Ok(not_found_error(
                    "Parent todo",
                    &pid.to_string(),
                    ToolGroup::Planning,
                ));
            }
        }
    }

    // 5. Validate subtasks if present
    if let Some(subtasks) = args.get("subtasks").and_then(|v| v.as_array()) {
        for (idx, subtask) in subtasks.iter().enumerate() {
            // Validate subtask title is non-empty
            let sub_title = subtask
                .get("title")
                .and_then(|v| v.as_str())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty());

            if sub_title.is_none() {
                return Ok(ErrorGuidance::with_guidance(
                    ErrorCategory::InvalidInput,
                    format!("Subtask at index {} has an empty or missing title", idx),
                    vec![
                        format!("Provide a non-empty title for subtask #{}", idx + 1),
                        "All subtasks must have non-empty titles".to_string(),
                        "Example: {\"title\": \"Implement X\"}".to_string(),
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

    let now = chrono::Utc::now().timestamp_millis();

    let result = sqlx::query(
        r#"
        INSERT INTO planning_todos (session_id, content, description, priority, parent_id, status, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, 'pending', ?, ?)
        "#,
    )
    .bind(session_id)
    .bind(title)
    .bind(description)
    .bind(priority)
    .bind(parent_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await;

    match result {
        Ok(query_result) => {
            let id = query_result.last_insert_rowid();

            // Handle subtasks if present
            if let Some(subtasks) = args.get("subtasks").and_then(|v| v.as_array()) {
                for subtask in subtasks {
                    // Title already validated in step 5, safe to unwrap
                    let sub_title = subtask
                        .get("title")
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim())
                        .unwrap_or("Untitled");
                    let sub_desc = subtask.get("description").and_then(|v| v.as_str());
                    let sub_prio = subtask
                        .get("priority")
                        .and_then(|v| v.as_str())
                        .unwrap_or("medium");

                    let _ = sqlx::query(
                        r#"
                        INSERT INTO planning_todos (session_id, content, description, priority, parent_id, status, created_at, updated_at)
                        VALUES (?, ?, ?, ?, ?, 'pending', ?, ?)
                        "#,
                    )
                    .bind(session_id)
                    .bind(sub_title)
                    .bind(sub_desc)
                    .bind(sub_prio)
                    .bind(id)
                    .bind(now)
                    .bind(now)
                    .execute(pool)
                    .await;
                }
            }

            let response_id = cuid2::create_id();
            let summary_text = get_planning_summary(pool, session_id).await;
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
pub async fn check_todo(
    pool: &SqlitePool,
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
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM planning_todos WHERE session_id = ? ORDER BY created_at ASC LIMIT 1 OFFSET ?"
        )
        .bind(session_id)
        .bind(idx)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to find todo by index: {}", e))?;

        match row {
            Some((tid,)) => tid,
            Option::None => {
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
        }
    } else {
        return Ok(missing_param_error("'id' or 'index'", ToolGroup::Planning));
    };

    let now = chrono::Utc::now().timestamp_millis();
    let status = if checked { "completed" } else { "pending" };

    // Store summary in description field if provided
    let result = if let Some(s) = summary {
        sqlx::query(
            r#"
            UPDATE planning_todos
            SET is_checked = ?, status = ?, description = COALESCE(description || ' - ' || ?, description, ?), updated_at = ?
            WHERE id = ? AND session_id = ?
            "#,
        )
        .bind(if checked { 1 } else { 0 })
        .bind(status)
        .bind(s)
        .bind(s)
        .bind(now)
        .bind(target_id)
        .bind(session_id)
        .execute(pool)
        .await
    } else {
        sqlx::query(
            r#"
            UPDATE planning_todos
            SET is_checked = ?, status = ?, updated_at = ?
            WHERE id = ? AND session_id = ?
            "#,
        )
        .bind(if checked { 1 } else { 0 })
        .bind(status)
        .bind(now)
        .bind(target_id)
        .bind(session_id)
        .execute(pool)
        .await
    };

    match result {
        Ok(_) => {
            let response_id = cuid2::create_id();
            let action = if checked { "checked" } else { "unchecked" };
            let summary_text = if let Some(s) = summary {
                format!(" - {}", s)
            } else {
                String::new()
            };
            let state_summary = get_planning_summary(pool, session_id).await;

            let next_todos: Vec<(i64, String)> = sqlx::query_as(
                "SELECT id, content FROM planning_todos WHERE session_id = ? AND is_checked = 0 ORDER BY id ASC LIMIT 3"
            )
            .bind(session_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            let next_actions = if next_todos.is_empty() {
                vec!["All todos checked! Use 'critiqueAndReflection' to review work, or 'createGoal' to start a new objective.".to_string()]
            } else {
                let mut actions = Vec::new();
                for (id, content) in next_todos {
                    // Truncate content if too long (safe unicode handling)
                    let safe_content = if content.chars().count() > 40 {
                        let truncated: String = content.chars().take(40).collect();
                        format!("{}...", truncated)
                    } else {
                        content
                    };
                    actions.push(format!("Process next: \"{}\" (ID: {})", safe_content, id));
                }
                actions
            };

            let hint = SuccessHint::new(
                format!(
                    "Todo {} (ID: {}){}{}",
                    action, target_id, summary_text, state_summary
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
                "Try again - this may be a transient error".to_string(),
                "Use getCurrentState to verify the todo exists".to_string(),
                "Verify the session is active".to_string(),
            ],
            ToolGroup::Planning,
        )
        .to_mcp_result()),
    }
}

/// Clear todos (Legacy: clearTodos)
pub async fn clear_todos(
    pool: &SqlitePool,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let ids = args.get("ids").and_then(|v| v.as_array());
    let indices = args.get("indices").and_then(|v| v.as_array());

    if ids.is_none() && indices.is_none() {
        // Clear all
        let result = sqlx::query("DELETE FROM planning_todos WHERE session_id = ?")
            .bind(session_id)
            .execute(pool)
            .await;

        return match result {
            Ok(r) => {
                let summary_text = get_planning_summary(pool, session_id).await;
                let hint = SuccessHint::new(
                    format!("✓ Cleared {} todos{}", r.rows_affected(), summary_text),
                    vec!["Use 'addTodo' to replan, or 'updateGoal'/'createGoal' to refine objectives".to_string()],
                );
                Ok(hint.to_mcp_result())
            }
            Err(e) => Ok(MCPResult::error(&format!("Failed to clear todos: {}", e))),
        };
    }

    let mut target_ids: Vec<i64> = Vec::new();

    // Collect explicit IDs
    if let Some(id_list) = ids {
        for id_val in id_list {
            if let Some(id) = id_val.as_i64() {
                if id < 1 {
                    return Ok(MCPResult::error(&format!(
                        "Invalid id '{}'. Must be >= 1",
                        id
                    )));
                }
                target_ids.push(id);
            }
        }
    }

    // Collect IDs from indices
    if let Some(idx_list) = indices {
        // Fetch all IDs ordered by created_at to map indices
        let all_todos: Vec<(i64,)> = sqlx::query_as(
            "SELECT id FROM planning_todos WHERE session_id = ? ORDER BY created_at ASC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch todos for index mapping: {}", e))?;

        for idx_val in idx_list {
            if let Some(idx) = idx_val.as_i64() {
                if idx < 0 {
                    return Ok(MCPResult::error(&format!(
                        "Invalid index '{}'. Must be >= 0",
                        idx
                    )));
                }
                let idx = idx as usize;
                if idx < all_todos.len() {
                    target_ids.push(all_todos[idx].0);
                }
            }
        }
    }

    if target_ids.is_empty() {
        return Ok(MCPResult::success("✓ No todos found to clear"));
    }

    // Remove duplicates
    target_ids.sort();
    target_ids.dedup();

    // Construct DELETE query with IN clause
    let placeholders: Vec<String> = target_ids.iter().map(|_| "?".to_string()).collect();
    let query = format!(
        "DELETE FROM planning_todos WHERE session_id = ? AND id IN ({})",
        placeholders.join(",")
    );

    let mut query_builder = sqlx::query(&query).bind(session_id);
    for id in target_ids {
        query_builder = query_builder.bind(id);
    }

    let result = query_builder.execute(pool).await;

    match result {
        Ok(r) => {
            let summary_text = get_planning_summary(pool, session_id).await;

            let next_todos: Vec<(i64, String)> = sqlx::query_as(
                "SELECT id, content FROM planning_todos WHERE session_id = ? AND is_checked = 0 ORDER BY id ASC LIMIT 3"
            )
            .bind(session_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            let next_actions = if next_todos.is_empty() {
                vec!["All todos cleared/checked! Use 'critiqueAndReflection' to review work, or 'createGoal' to start a new objective.".to_string()]
            } else {
                let mut actions = Vec::new();
                for (id, content) in next_todos {
                    let safe_content = if content.chars().count() > 40 {
                        let truncated: String = content.chars().take(40).collect();
                        format!("{}...", truncated)
                    } else {
                        content
                    };
                    actions.push(format!("Process next: \"{}\" (ID: {})", safe_content, id));
                }
                actions
            };

            let hint = SuccessHint::new(
                format!("✓ Cleared {} todos{}", r.rows_affected(), summary_text),
                next_actions,
            );
            Ok(hint.to_mcp_result())
        }
        Err(e) => Ok(MCPResult::error(&format!("Failed to clear todos: {}", e))),
    }
}
