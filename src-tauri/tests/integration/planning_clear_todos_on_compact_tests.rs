use crate::common;

use tauri_mcp_agent_lib::agent::planning_reset::clear_todos_after_successful_compaction;
use tauri_mcp_agent_lib::repositories::{PlanningRepository, SqlitePlanningRepository};
use tauri_mcp_agent_lib::set_planning_repository;

#[tokio::test]
async fn clear_todos_after_successful_compaction_keeps_goal_and_scratchpad() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqlitePlanningRepository::new(db.clone());
    set_planning_repository(repo);

    let session_id = "planning-clear-todos-on-compact";
    let planning = SqlitePlanningRepository::new(db);

    planning
        .create_goal(session_id, "Ship the release notes")
        .await
        .expect("goal should be created");
    planning
        .add_todo(
            session_id,
            "Draft changelog",
            Some("Write the user-facing notes".to_string()),
            "high",
        )
        .await
        .expect("todo should be created");
    planning
        .add_todo(session_id, "Proofread", None, "medium")
        .await
        .expect("second todo should be created");
    planning
        .add_scratchpad(
            session_id,
            Some("release-ids".to_string()),
            "PR-1234",
            None,
            None,
        )
        .await
        .expect("scratchpad note should be created");

    clear_todos_after_successful_compaction(session_id).await;

    let goal = planning
        .get_active_goal(session_id)
        .await
        .expect("goal lookup should succeed")
        .expect("goal must remain after todo clear");
    assert_eq!(goal.goal_text, "Ship the release notes");

    let todos = planning
        .list_todos(session_id, true)
        .await
        .expect("todo list should succeed");
    assert!(
        todos.is_empty(),
        "all todos should be cleared after compaction helper"
    );

    let notes = planning
        .list_scratchpad(session_id)
        .await
        .expect("scratchpad list should succeed");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].title.as_deref(), Some("release-ids"));
}

#[tokio::test]
async fn clear_session_still_wipes_goal_todos_and_scratchpad() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqlitePlanningRepository::new(db.clone());
    set_planning_repository(repo);

    let session_id = "planning-clear-session-full-wipe";
    let planning = SqlitePlanningRepository::new(db);

    planning
        .create_goal(session_id, "Full wipe target")
        .await
        .expect("goal should be created");
    planning
        .add_todo(session_id, "Temp task", None, "low")
        .await
        .expect("todo should be created");
    planning
        .add_scratchpad(session_id, Some("note".to_string()), "body", None, None)
        .await
        .expect("scratchpad note should be created");

    planning
        .clear_session(session_id)
        .await
        .expect("clear_session should succeed");

    assert!(planning
        .get_active_goal(session_id)
        .await
        .expect("goal lookup")
        .is_none());
    assert!(planning
        .list_todos(session_id, true)
        .await
        .expect("todos")
        .is_empty());
    assert!(planning
        .list_scratchpad(session_id)
        .await
        .expect("scratchpad")
        .is_empty());
}
