use crate::common;

use serde_json::json;
use tauri_mcp_agent_lib::mcp::builtin::ui::UiServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::repositories::{PlanningRepository, SqlitePlanningRepository};
use tauri_mcp_agent_lib::set_planning_repository;

async fn seed_planning(session_id: &str, planning: &SqlitePlanningRepository) {
    planning
        .create_goal(session_id, "Finish the deliverable")
        .await
        .expect("goal should be created");
    planning
        .add_todo(session_id, "Write the file", None, "high")
        .await
        .expect("todo should be created");
    planning
        .add_scratchpad(
            session_id,
            Some("paths".to_string()),
            "output/report.md",
            None,
            None,
        )
        .await
        .expect("scratchpad note should be created");
}

async fn assert_planning_empty(session_id: &str, planning: &SqlitePlanningRepository) {
    assert!(
        planning
            .get_active_goal(session_id)
            .await
            .expect("goal lookup")
            .is_none(),
        "goal should be cleared"
    );
    assert!(
        planning
            .list_todos(session_id, true)
            .await
            .expect("todos")
            .is_empty(),
        "todos should be cleared"
    );
    assert!(
        planning
            .list_scratchpad(session_id)
            .await
            .expect("scratchpad")
            .is_empty(),
        "scratchpad should be cleared"
    );
}

async fn assert_planning_intact(session_id: &str, planning: &SqlitePlanningRepository) {
    let goal = planning
        .get_active_goal(session_id)
        .await
        .expect("goal lookup")
        .expect("goal must remain");
    assert_eq!(goal.goal_text, "Finish the deliverable");
    assert_eq!(
        planning
            .list_todos(session_id, true)
            .await
            .expect("todos")
            .len(),
        1
    );
    assert_eq!(
        planning
            .list_scratchpad(session_id)
            .await
            .expect("scratchpad")
            .len(),
        1
    );
}

#[tokio::test]
async fn report_result_success_clears_goal_todos_and_scratchpad() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqlitePlanningRepository::new(db.clone());
    set_planning_repository(repo);

    let session_id = "planning-clear-on-report-result-success";
    let planning = SqlitePlanningRepository::new(db);
    seed_planning(session_id, &planning).await;

    let server = UiServer::new();
    let result = server
        .call_tool(
            "reportResult",
            json!({
                "status": "success",
                "result": "Deliverable ready."
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("reportResult success should execute");

    assert_eq!(result.is_error, Some(false));
    assert_planning_empty(session_id, &planning).await;
}

#[tokio::test]
async fn report_result_partial_keeps_planning_state() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqlitePlanningRepository::new(db.clone());
    set_planning_repository(repo);

    let session_id = "planning-clear-on-report-result-partial";
    let planning = SqlitePlanningRepository::new(db);
    seed_planning(session_id, &planning).await;

    let server = UiServer::new();
    let result = server
        .call_tool(
            "reportResult",
            json!({
                "status": "partial",
                "result": "Best-effort output with known gaps."
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("reportResult partial should execute");

    assert_eq!(result.is_error, Some(false));
    assert_planning_intact(session_id, &planning).await;
}

#[tokio::test]
async fn report_result_success_without_session_id_skips_planning_clear() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqlitePlanningRepository::new(db.clone());
    set_planning_repository(repo);

    let session_id = "planning-clear-on-report-result-no-session";
    let planning = SqlitePlanningRepository::new(db);
    seed_planning(session_id, &planning).await;

    let server = UiServer::new();
    let result = server
        .call_tool(
            "reportResult",
            json!({
                "status": "success",
                "result": "Done without session binding."
            }),
            None,
        )
        .await
        .expect("reportResult without session_id should execute");

    assert_eq!(result.is_error, Some(false));
    // No session_id → clear is skipped (cannot target a session).
    assert_planning_intact(session_id, &planning).await;
}
