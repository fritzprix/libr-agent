use super::errors::{planning_follow_up_read_notice, planning_write_error};
use crate::mcp::builtin::error_guidance::{missing_param_error, SuccessHint, ToolGroup};
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
            let mut next_hints = vec![
                "Use addTodo to break down this goal into tasks".to_string(),
                "Use getCurrentState to review the full plan".to_string(),
            ];
            let summary = match repo.get_planning_summary(session_id).await {
                Ok(summary) => summary,
                Err(error) => {
                    let notice = planning_follow_up_read_notice("updated planning summary", &error);
                    next_hints.push(notice.hint);
                    notice.suffix
                }
            };

            let hint = SuccessHint::new(
                format!("✓ Goal created: {}{}", goal_text, summary),
                next_hints,
            );

            Ok(hint.to_mcp_result_with_data(Some(json!({
                "id": response_id,
                "success": true,
                "goal": goal_text,
                "goalId": id
            }))))
        }
        Err(e) => Ok(planning_write_error(
            "create the goal",
            &e,
            vec![
                "Use getCurrentState to verify whether a goal is already active.".to_string(),
                "Retry only if the goal was not created.".to_string(),
            ],
        )),
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
                let mut next_hints = vec![
                    "Use addTodo to add tasks for this updated goal".to_string(),
                    "Use getCurrentState to review changes".to_string(),
                ];
                let summary = match repo.get_planning_summary(session_id).await {
                    Ok(summary) => summary,
                    Err(error) => {
                        let notice =
                            planning_follow_up_read_notice("updated planning summary", &error);
                        next_hints.push(notice.hint);
                        notice.suffix
                    }
                };

                let hint = SuccessHint::new(
                    format!("✓ Goal updated: {}{}", goal_text, summary),
                    next_hints,
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
        Err(e) => Ok(planning_write_error(
            "update the active goal",
            &e,
            vec![
                "Use getCurrentState to confirm whether the goal changed.".to_string(),
                "Use createGoal if no goal exists yet.".to_string(),
            ],
        )),
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
        Err(e) => Ok(planning_write_error(
            "clear the active goal",
            &e,
            vec![
                "Use getCurrentState to see whether the goal is still active.".to_string(),
                "Retry only if the goal was not cleared.".to_string(),
            ],
        )),
    }
}
