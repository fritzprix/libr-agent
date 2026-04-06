mod common;

use tauri_mcp_agent_lib::repositories::SqliteScheduledTaskRepository;
use tauri_mcp_agent_lib::services::scheduled_task_service::{
    CreateScheduledTaskInput, ScheduledTaskGovernanceSettings, ScheduledTaskService,
};

fn create_input(
    name: &str,
    group_name: Option<&str>,
    group_id: Option<&str>,
    cron: &str,
) -> CreateScheduledTaskInput {
    CreateScheduledTaskInput {
        name: name.to_string(),
        cron_expression: cron.to_string(),
        schedule_timezone: "local".to_string(),
        assistant_id: "assistant-1".to_string(),
        group_id: group_id.map(ToString::to_string),
        group_name: group_name.map(ToString::to_string),
        message: format!("Run task {}", name),
        yolo_mode: false,
        created_by_session_id: Some("session-origin".to_string()),
        workspace_override: None,
    }
}

#[tokio::test]
async fn create_scheduled_task_rejects_intervals_below_governed_minimum() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteScheduledTaskRepository::new(db);
    let governance = ScheduledTaskGovernanceSettings {
        minimum_interval_minutes: 5,
        max_scheduled_task_groups: 10,
    };

    let error = ScheduledTaskService::create_scheduled_task_with_governance(
        &repo,
        create_input("Too Frequent", None, None, "* * * * *"),
        &governance,
    )
    .await
    .expect_err("min-interval policy should reject every-minute schedules");

    assert!(error.contains("Minimum allowed interval is 5 minute(s)"));
}

#[tokio::test]
async fn create_scheduled_task_enforces_distinct_group_limit() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteScheduledTaskRepository::new(db);
    let governance = ScheduledTaskGovernanceSettings {
        minimum_interval_minutes: 0,
        max_scheduled_task_groups: 2,
    };

    ScheduledTaskService::create_scheduled_task_with_governance(
        &repo,
        create_input("Research", Some("Research"), Some("research"), "0 * * * *"),
        &governance,
    )
    .await
    .expect("first group should be allowed");
    ScheduledTaskService::create_scheduled_task_with_governance(
        &repo,
        create_input("Analysis", Some("Analysis"), Some("analysis"), "5 * * * *"),
        &governance,
    )
    .await
    .expect("second group should be allowed");

    let error = ScheduledTaskService::create_scheduled_task_with_governance(
        &repo,
        create_input(
            "Reporting",
            Some("Reporting"),
            Some("reporting"),
            "10 * * * *",
        ),
        &governance,
    )
    .await
    .expect_err("third distinct group should exceed governance cap");

    assert!(error.contains("Maximum scheduled task groups reached"));
}

#[tokio::test]
async fn create_scheduled_task_persists_group_and_provenance_metadata() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteScheduledTaskRepository::new(db);

    let created = ScheduledTaskService::create_scheduled_task_with_governance(
        &repo,
        create_input(
            "Grouped Task",
            Some("Research Team"),
            Some("research-team"),
            "0 9 * * *",
        ),
        &ScheduledTaskGovernanceSettings::default(),
    )
    .await
    .expect("grouped task should be created");

    assert_eq!(created.group_id.as_deref(), Some("research-team"));
    assert_eq!(created.group_name.as_deref(), Some("Research Team"));
    assert_eq!(
        created.created_by_session_id.as_deref(),
        Some("session-origin")
    );
}
