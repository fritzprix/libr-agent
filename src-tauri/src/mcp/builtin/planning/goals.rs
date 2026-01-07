use crate::entity::planning_goal;
use crate::mcp::builtin::planning::context::get_planning_summary;
use crate::mcp::types::MCPResult;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};

/// Create a new goal (Legacy: createGoal)
pub async fn create_goal(
    db: &DatabaseConnection,
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
    planning_goal::Entity::update_many()
        .col_expr(
            planning_goal::Column::Status,
            sea_orm::sea_query::Expr::value("archived"),
        )
        .filter(planning_goal::Column::SessionId.eq(session_id))
        .filter(planning_goal::Column::Status.eq("active"))
        .exec(db)
        .await
        .map_err(|e| format!("Failed to archive old goals: {}", e))?;

    // Insert new goal
    let new_goal = planning_goal::ActiveModel {
        session_id: Set(session_id.to_string()),
        goal_text: Set(goal.to_string()),
        status: Set("active".to_string()),
        created_at: Set(now),
        ..Default::default()
    };

    match new_goal.insert(db).await {
        Ok(model) => {
            let id = model.id;
            let response_id = cuid2::create_id();
            // Since we just created a goal, we know the goal text.
            // We can fetch summary to get todo counts.
            let summary = get_planning_summary(db, session_id).await;

            Ok(MCPResult::success_with_data(
                &format!("✓ Goal created: {}{}\n\nNow break this down into actionable tasks using addTodo.", goal, summary),
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
    db: &DatabaseConnection,
    session_id: &str,
    args: Value,
) -> Result<MCPResult, String> {
    let goal = args
        .get("goal")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Missing or empty 'goal' parameter".to_string())?;

    let result = planning_goal::Entity::update_many()
        .col_expr(
            planning_goal::Column::GoalText,
            sea_orm::sea_query::Expr::value(goal),
        )
        .filter(planning_goal::Column::SessionId.eq(session_id))
        .filter(planning_goal::Column::Status.eq("active"))
        .exec(db)
        .await;

    match result {
        Ok(update_result) => {
            if update_result.rows_affected > 0 {
                let response_id = cuid2::create_id();
                let summary = get_planning_summary(db, session_id).await;
                Ok(MCPResult::success_with_data(
                    &format!("✓ Goal updated: {}{}", goal, summary),
                    json!({
                        "id": response_id,
                        "success": true,
                        "goal": goal
                    }),
                ))
            } else {
                // If no active goal, create one
                create_goal(db, session_id, args).await
            }
        }
        Err(e) => Ok(MCPResult::error(&format!("Failed to update goal: {}", e))),
    }
}

/// Clear current goal (Legacy: clearGoal)
pub async fn clear_goal(
    db: &DatabaseConnection,
    session_id: &str,
    _args: Value,
) -> Result<MCPResult, String> {
    let result = planning_goal::Entity::update_many()
        .col_expr(
            planning_goal::Column::Status,
            sea_orm::sea_query::Expr::value("cleared"),
        )
        .filter(planning_goal::Column::SessionId.eq(session_id))
        .filter(planning_goal::Column::Status.eq("active"))
        .exec(db)
        .await;

    match result {
        Ok(_) => Ok(MCPResult::success("✓ Goal cleared")),
        Err(e) => Ok(MCPResult::error(&format!("Failed to clear goal: {}", e))),
    }
}
