use crate::entity::planning_goal;
use crate::mcp::builtin::error_guidance::{
    missing_param_error, ErrorCategory, ErrorGuidance, SuccessHint, ToolGroup,
};
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
        .filter(|s| !s.is_empty());

    let goal_text = match goal {
        Some(g) => g,
        None => {
            return Ok(missing_param_error("goal", ToolGroup::Planning));
        }
    };

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
        goal_text: Set(goal_text.to_string()),
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

            let hint = SuccessHint::new(
                format!("✓ Goal created: {}{}", goal_text, summary),
                vec![
                    "Use addTodo to break down this goal into tasks".to_string(),
                    "Use getCurrentState to review the full plan".to_string(),
                ],
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "id": response_id,
                "success": true,
                "goal": goal_text,
                "goalId": id
            }))))
        }
        Err(e) => Ok(ErrorGuidance::with_guidance(
            ErrorCategory::DatabaseError,
            format!("Failed to create goal: {}", e),
            vec![
                "Try again - this may be a transient error".to_string(),
                "Use getCurrentState to check if a goal is already active".to_string(),
            ],
            ToolGroup::Planning,
        )
        .to_mcp_result()),
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
        .filter(|s| !s.is_empty());

    let goal_text = match goal {
        Some(g) => g,
        None => {
            return Ok(missing_param_error("goal", ToolGroup::Planning));
        }
    };

    let result = planning_goal::Entity::update_many()
        .col_expr(
            planning_goal::Column::GoalText,
            sea_orm::sea_query::Expr::value(goal_text),
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

                let hint = SuccessHint::new(
                    format!("✓ Goal updated: {}{}", goal_text, summary),
                    vec![
                        "Use addTodo to add tasks for this updated goal".to_string(),
                        "Use getCurrentState to review changes".to_string(),
                    ],
                );

                Ok(hint.to_mcp_result_with_data(Some(json!({
                    "id": response_id,
                    "success": true,
                    "goal": goal_text
                }))))
            } else {
                // If no active goal, create one
                create_goal(db, session_id, args).await
            }
        }
        Err(e) => Ok(ErrorGuidance::with_guidance(
            ErrorCategory::DatabaseError,
            format!("Failed to update goal: {}", e),
            vec![
                "Try again - database might be busy".to_string(),
                "Use createGoal if no goal exists yet".to_string(),
            ],
            ToolGroup::Planning,
        )
        .to_mcp_result()),
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
        Ok(_) => {
            let hint = SuccessHint::new(
                "✓ Goal cleared",
                vec!["Use createGoal to set a new objective".to_string()],
            );
            Ok(hint.to_mcp_result())
        }
        Err(e) => Ok(ErrorGuidance::with_guidance(
            ErrorCategory::DatabaseError,
            format!("Failed to clear goal: {}", e),
            vec![
                "Try again".to_string(),
                "Use getCurrentState to see if it was already cleared".to_string(),
            ],
            ToolGroup::Planning,
        )
        .to_mcp_result()),
    }
}
