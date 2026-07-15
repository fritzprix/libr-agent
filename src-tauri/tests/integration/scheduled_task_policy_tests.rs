use crate::common;

use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::repositories::SqliteScheduledTaskRepository;
use tauri_mcp_agent_lib::scheduled::TASK_CATEGORY_GLOBAL;
use tauri_mcp_agent_lib::services::scheduled_task_service::{
    CreateScheduledTaskInput, ScheduledTaskGovernanceSettings, ScheduledTaskService,
};

fn create_input(name: &str, cron: &str) -> CreateScheduledTaskInput {
    CreateScheduledTaskInput {
        name: name.to_string(),
        task_category: TASK_CATEGORY_GLOBAL.to_string(),
        cron_expression: Some(cron.to_string()),
        schedule_timezone: "local".to_string(),
        assistant_id: "assistant-1".to_string(),
        message: format!("Run task {}", name),
        execution_mode: ExecutionMode::Normal,
        created_by_session_id: Some("session-origin".to_string()),
        session_id: None,
        workspace_override: None,
        reset_planning_state: false,
        next_run_at: None,
    }
}

#[tokio::test]
async fn create_scheduled_task_rejects_intervals_below_governed_minimum() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteScheduledTaskRepository::new(db);
    let governance = ScheduledTaskGovernanceSettings {
        minimum_interval_minutes: 5,
    };

    let error = ScheduledTaskService::create_scheduled_task_with_governance(
        &repo,
        create_input("Too Frequent", "* * * * *"),
        &governance,
    )
    .await
    .expect_err("min-interval policy should reject every-minute schedules");

    assert!(error.contains("Minimum allowed interval is 5 minute(s)"));
}

#[tokio::test]
async fn create_scheduled_task_persists_provenance_metadata() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteScheduledTaskRepository::new(db);

    let created = ScheduledTaskService::create_scheduled_task_with_governance(
        &repo,
        create_input("Daily Task", "0 9 * * *"),
        &ScheduledTaskGovernanceSettings::default(),
    )
    .await
    .expect("task should be created");

    assert_eq!(
        created.created_by_session_id.as_deref(),
        Some("session-origin")
    );
}

#[tokio::test]
async fn create_scheduled_task_persists_execution_mode() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteScheduledTaskRepository::new(db);

    let mut input = create_input("Unsafe Task", "0 10 * * *");
    input.execution_mode = ExecutionMode::Unsafe;

    let created = ScheduledTaskService::create_scheduled_task_with_governance(
        &repo,
        input,
        &ScheduledTaskGovernanceSettings::default(),
    )
    .await
    .expect("unsafe task should be created");

    assert_eq!(created.execution_mode(), ExecutionMode::Unsafe);
}
