mod common;

use tauri_mcp_agent_lib::repositories::{
    ScheduledTaskRepository, SessionMetadata, SessionRepository, SessionStatus,
    SqliteScheduledTaskRepository, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::scheduled::runner::sync_task_workspace_override;
use tauri_mcp_agent_lib::services::scheduled_task_service::{
    CreateScheduledTaskInput, ScheduledTaskGovernanceSettings, ScheduledTaskService,
};
use tauri_mcp_agent_lib::set_session_repository;

fn make_session(session_id: &str) -> SessionMetadata {
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Scheduled Task Session".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-4.1".to_string(),
        provider: "openai".to_string(),
        agent_config: None,
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
async fn sync_task_workspace_override_clears_stale_scheduled_override() {
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    set_session_repository(session_repo.clone());
    let scheduled_repo = SqliteScheduledTaskRepository::new(db);

    let session_id = "scheduled-stale-workspace-session";
    session_repo
        .upsert_session(&make_session(session_id))
        .await
        .expect("session should be persisted");

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let missing_workspace = temp_dir.path().join("missing-workspace");
    let missing_workspace_str = missing_workspace.to_string_lossy().to_string();

    let task = ScheduledTaskService::create_scheduled_task_with_governance(
        &scheduled_repo,
        CreateScheduledTaskInput {
            name: "Nightly report".to_string(),
            cron_expression: "0 9 * * *".to_string(),
            schedule_timezone: "local".to_string(),
            assistant_id: "assistant-1".to_string(),
            group_id: None,
            group_name: None,
            message: "Generate report".to_string(),
            yolo_mode: false,
            created_by_session_id: Some(session_id.to_string()),
            workspace_override: Some(missing_workspace_str.clone()),
        },
        &ScheduledTaskGovernanceSettings::default(),
    )
    .await
    .expect("scheduled task should be created");

    sync_task_workspace_override(
        &scheduled_repo,
        &task.id,
        &task.name,
        session_id,
        task.workspace_override.as_deref(),
    )
    .await
    .expect("stale override should be cleared instead of failing");

    let updated = scheduled_repo
        .get_scheduled_task(&task.id)
        .await
        .expect("task lookup should succeed")
        .expect("task should still exist");

    assert_eq!(updated.workspace_override, None);
}
