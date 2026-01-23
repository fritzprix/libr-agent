use crate::mcp::builtin::error_guidance::{
    missing_param_error, ErrorCategory, ErrorGuidance, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::PlanningRepository;
use crate::state::get_planning_repository;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

/// Create a new goal (Legacy: createGoal)
pub async fn create_goal(
    _db: &DatabaseConnection,
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

    let repo = get_planning_repository();

    match repo.create_goal(session_id, goal_text).await {
        Ok(id) => {
            let response_id = cuid2::create_id();
            let summary = repo
                .get_planning_summary(session_id)
                .await
                .unwrap_or_default();

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

    let repo = get_planning_repository();

    match repo.update_goal(session_id, goal_text).await {
        Ok(updated) => {
            if updated {
                let response_id = cuid2::create_id();
                let summary = repo
                    .get_planning_summary(session_id)
                    .await
                    .unwrap_or_default();

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
    _db: &DatabaseConnection,
    session_id: &str,
    _args: Value,
) -> Result<MCPResult, String> {
    let repo = get_planning_repository();

    match repo.clear_goal(session_id).await {
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
