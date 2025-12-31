use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use sqlx::SqlitePool;

/// Create a new goal (Legacy: createGoal)
pub async fn create_goal(
    pool: &SqlitePool,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let goal = args
        .get("goal")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Missing or empty 'goal' parameter".to_string())?;

    let now = chrono::Utc::now().timestamp_millis();

    // Deactivate existing active goals
    sqlx::query(
        "UPDATE planning_goals SET status = 'archived' WHERE session_id = ? AND status = 'active'",
    )
    .bind(session_id)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to archive old goals: {}", e))?;

    // Insert new goal
    let result = sqlx::query(
        r#"
        INSERT INTO planning_goals (session_id, goal_text, status, created_at)
        VALUES (?, ?, 'active', ?)
        "#,
    )
    .bind(session_id)
    .bind(goal)
    .bind(now)
    .execute(pool)
    .await;

    match result {
        Ok(query_result) => {
            let id = query_result.last_insert_rowid();
            let response_id = cuid2::create_id();
            Ok(MCPResult::success_with_data(
                &format!("✓ Goal created: {}\n\nNow break this down into actionable tasks using addTodo.", goal),
                json!({
                    "id": response_id,
                    "success": true,
                    "goal": goal,
                    "goalId": id
                }),
            ))
        }
        Err(e) => Ok(MCPResult::error(&format!("Failed to create goal: {}", e))),
    }
}

/// Update current goal (Legacy: updateGoal)
pub async fn update_goal(
    pool: &SqlitePool,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let goal = args
        .get("goal")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Missing or empty 'goal' parameter".to_string())?;

    let result = sqlx::query(
        r#"
        UPDATE planning_goals 
        SET goal_text = ? 
        WHERE session_id = ? AND status = 'active'
        "#,
    )
    .bind(goal)
    .bind(session_id)
    .execute(pool)
    .await;

    match result {
        Ok(query_result) => {
            if query_result.rows_affected() > 0 {
                let response_id = cuid2::create_id();
                Ok(MCPResult::success_with_data(
                    &format!("✓ Goal updated: {}", goal),
                    json!({
                        "id": response_id,
                        "success": true,
                        "goal": goal
                    }),
                ))
            } else {
                // If no active goal, create one
                create_goal(pool, session_id, args).await
            }
        }
        Err(e) => Ok(MCPResult::error(&format!("Failed to update goal: {}", e))),
    }
}

/// Clear current goal (Legacy: clearGoal)
pub async fn clear_goal(
    pool: &SqlitePool,
    session_id: &str,
    _args: Value,
) -> Result<MCPResult, String> {
    let result = sqlx::query(
        r#"
        UPDATE planning_goals 
        SET status = 'cleared' 
        WHERE session_id = ? AND status = 'active'
        "#,
    )
    .bind(session_id)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(MCPResult::success("✓ Goal cleared")),
        Err(e) => Ok(MCPResult::error(&format!("Failed to clear goal: {}", e))),
    }
}
