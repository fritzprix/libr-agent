use crate::common;

use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteScheduledTaskRepository,
    SqliteSessionRepository,
};
use tauri_mcp_agent_lib::scheduled::{TASK_CATEGORY_GLOBAL, TASK_CATEGORY_SESSION};
use tauri_mcp_agent_lib::services::scheduled_task_service::{
    CreateScheduledTaskInput, ScheduledTaskGovernanceSettings, ScheduledTaskService,
};

fn make_session(session_id: &str, assistant_id: &str) -> SessionMetadata {
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Callback Session".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-4.1".to_string(),
        provider: "openai".to_string(),
        agent_config: Some(
            serde_json::json!({
                "assistantId": assistant_id,
                "name": "Test Assistant",
                "systemPrompt": "test"
            })
            .to_string(),
        ),
        parent_session_id: None,
        lineage_id: None,
        depth: None,
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: 1,
        updated_at: 1,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        yolo_mode: false,
        unsafe_mode: false,
        workspace_override: None,
    }
}

#[tokio::test]
async fn create_session_one_shot_callback_pins_session_and_skips_interval_policy() {
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let scheduled_repo = SqliteScheduledTaskRepository::new(db);

    let session_id = "session-callback-one-shot";
    session_repo
        .upsert_session(&make_session(session_id, "assistant-1"))
        .await
        .expect("session should be persisted");

    let now_ms = chrono::Utc::now().timestamp_millis();
    let governance = ScheduledTaskGovernanceSettings {
        minimum_interval_minutes: 30,
        max_scheduled_task_groups: 10,
    };

    let created = ScheduledTaskService::create_scheduled_task_with_governance(
        &scheduled_repo,
        CreateScheduledTaskInput {
            name: "Check back soon".to_string(),
            task_category: TASK_CATEGORY_SESSION.to_string(),
            cron_expression: None,
            schedule_timezone: "local".to_string(),
            assistant_id: "assistant-1".to_string(),
            group_id: None,
            group_name: None,
            message: "Follow up on this thread".to_string(),
            yolo_mode: false,
            created_by_session_id: Some(session_id.to_string()),
            session_id: Some(session_id.to_string()),
            workspace_override: None,
            next_run_at: Some(now_ms + 5_000),
        },
        &governance,
    )
    .await
    .expect("one-shot SESSION callback should bypass cron interval policy");

    assert_eq!(created.task_category, TASK_CATEGORY_SESSION);
    assert_eq!(created.session_id.as_deref(), Some(session_id));
    assert!(created.cron_expression.is_none());
    assert_eq!(created.next_run_at, Some(now_ms + 5_000));
}

#[tokio::test]
async fn create_session_callback_requires_session_id() {
    let db = common::setup_test_db_with_migrations().await;
    let scheduled_repo = SqliteScheduledTaskRepository::new(db);
    let now_ms = chrono::Utc::now().timestamp_millis();

    let error = ScheduledTaskService::create_scheduled_task_with_governance(
        &scheduled_repo,
        CreateScheduledTaskInput {
            name: "Missing session".to_string(),
            task_category: TASK_CATEGORY_SESSION.to_string(),
            cron_expression: None,
            schedule_timezone: "local".to_string(),
            assistant_id: "assistant-1".to_string(),
            group_id: None,
            group_name: None,
            message: "noop".to_string(),
            yolo_mode: false,
            created_by_session_id: None,
            session_id: None,
            workspace_override: None,
            next_run_at: Some(now_ms + 1_000),
        },
        &ScheduledTaskGovernanceSettings::default(),
    )
    .await
    .expect_err("SESSION tasks without session_id should fail");

    assert!(error.contains("session_id"));
}

#[tokio::test]
async fn global_tasks_still_require_cron_expression() {
    let db = common::setup_test_db_with_migrations().await;
    let scheduled_repo = SqliteScheduledTaskRepository::new(db);

    let error = ScheduledTaskService::create_scheduled_task_with_governance(
        &scheduled_repo,
        CreateScheduledTaskInput {
            name: "Broken global".to_string(),
            task_category: TASK_CATEGORY_GLOBAL.to_string(),
            cron_expression: None,
            schedule_timezone: "local".to_string(),
            assistant_id: "assistant-1".to_string(),
            group_id: None,
            group_name: None,
            message: "noop".to_string(),
            yolo_mode: false,
            created_by_session_id: None,
            session_id: None,
            workspace_override: None,
            next_run_at: None,
        },
        &ScheduledTaskGovernanceSettings::default(),
    )
    .await
    .expect_err("GLOBAL tasks without cron should fail");

    assert!(error.contains("cron_expression"));
}
