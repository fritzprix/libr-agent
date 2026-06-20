use crate::common;

use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::repositories::{
    ScheduledTaskRepository, SessionMetadata, SessionRepository, SessionStatus,
    SqliteScheduledTaskRepository, SqliteSessionRepository,
};

fn extract_text_content(result: &tauri_mcp_agent_lib::mcp::types::MCPResult) -> String {
    result
        .content
        .as_ref()
        .expect("text content expected")
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
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
        assistant_id: Some(assistant_id.to_string()),
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
            unsafe_mode: false,
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
            unsafe_mode: false,
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
            unsafe_mode: false,
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

#[tokio::test]
async fn delete_session_scheduled_tasks_removes_only_session_callbacks() {
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let scheduled_repo = SqliteScheduledTaskRepository::new(db);

    let session_id = "session-delete-callbacks";
    session_repo
        .upsert_session(&make_session(session_id, "assistant-1"))
        .await
        .expect("session should be persisted");

    let now_ms = chrono::Utc::now().timestamp_millis();
    let governance = ScheduledTaskGovernanceSettings::default();

    let session_task = ScheduledTaskService::create_scheduled_task_with_governance(
        &scheduled_repo,
        CreateScheduledTaskInput {
            name: "Session callback".to_string(),
            task_category: TASK_CATEGORY_SESSION.to_string(),
            cron_expression: None,
            schedule_timezone: "local".to_string(),
            assistant_id: "assistant-1".to_string(),
            group_id: None,
            group_name: None,
            message: "Follow up".to_string(),
            yolo_mode: false,
            unsafe_mode: false,
            created_by_session_id: Some(session_id.to_string()),
            session_id: Some(session_id.to_string()),
            workspace_override: None,
            next_run_at: Some(now_ms + 60_000),
        },
        &governance,
    )
    .await
    .expect("session callback should be created");

    let global_task = ScheduledTaskService::create_scheduled_task_with_governance(
        &scheduled_repo,
        CreateScheduledTaskInput {
            name: "Global recurring".to_string(),
            task_category: TASK_CATEGORY_GLOBAL.to_string(),
            cron_expression: Some("0 9 * * *".to_string()),
            schedule_timezone: "local".to_string(),
            assistant_id: "assistant-1".to_string(),
            group_id: None,
            group_name: None,
            message: "Daily check".to_string(),
            yolo_mode: false,
            unsafe_mode: false,
            created_by_session_id: None,
            session_id: Some(session_id.to_string()),
            workspace_override: None,
            next_run_at: None,
        },
        &governance,
    )
    .await
    .expect("global task should be created");

    let deleted = ScheduledTaskService::delete_session_scheduled_tasks_for_sessions(
        &scheduled_repo,
        &[session_id.to_string()],
    )
    .await
    .expect("session callback cleanup should succeed");

    assert_eq!(deleted, 1);
    assert!(scheduled_repo
        .get_scheduled_task(&session_task.id)
        .await
        .expect("lookup should succeed")
        .is_none());
    assert!(scheduled_repo
        .get_scheduled_task(&global_task.id)
        .await
        .expect("lookup should succeed")
        .is_some());
}

#[tokio::test]
async fn list_session_scheduled_tasks_returns_enabled_session_callbacks_only() {
    let db = common::setup_test_db_with_migrations().await;
    let scheduled_repo = SqliteScheduledTaskRepository::new(db);

    let session_id = "session-list-callbacks";
    let now_ms = chrono::Utc::now().timestamp_millis();
    let governance = ScheduledTaskGovernanceSettings::default();

    ScheduledTaskService::create_scheduled_task_with_governance(
        &scheduled_repo,
        CreateScheduledTaskInput {
            name: "Active callback".to_string(),
            task_category: TASK_CATEGORY_SESSION.to_string(),
            cron_expression: None,
            schedule_timezone: "local".to_string(),
            assistant_id: "assistant-1".to_string(),
            group_id: None,
            group_name: None,
            message: "Ping".to_string(),
            yolo_mode: false,
            unsafe_mode: false,
            created_by_session_id: Some(session_id.to_string()),
            session_id: Some(session_id.to_string()),
            workspace_override: None,
            next_run_at: Some(now_ms + 30_000),
        },
        &governance,
    )
    .await
    .expect("active callback should be created");

    ScheduledTaskService::create_scheduled_task_with_governance(
        &scheduled_repo,
        CreateScheduledTaskInput {
            name: "Other session callback".to_string(),
            task_category: TASK_CATEGORY_SESSION.to_string(),
            cron_expression: None,
            schedule_timezone: "local".to_string(),
            assistant_id: "assistant-1".to_string(),
            group_id: None,
            group_name: None,
            message: "Other".to_string(),
            yolo_mode: false,
            unsafe_mode: false,
            created_by_session_id: Some("other-session".to_string()),
            session_id: Some("other-session".to_string()),
            workspace_override: None,
            next_run_at: Some(now_ms + 30_000),
        },
        &governance,
    )
    .await
    .expect("other callback should be created");

    let listed = ScheduledTaskService::list_session_scheduled_tasks(&scheduled_repo, session_id)
        .await
        .expect("list should succeed");

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "Active callback");
}

#[tokio::test]
async fn cascade_cleanup_deletes_callbacks_pinned_to_descendant_sessions() {
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let scheduled_repo = SqliteScheduledTaskRepository::new(db);

    let parent_id = "parent-session";
    let child_id = "child-session";
    session_repo
        .upsert_session(&make_session(parent_id, "assistant-1"))
        .await
        .expect("parent session should be persisted");

    let mut child = make_session(child_id, "assistant-1");
    child.parent_session_id = Some(parent_id.to_string());
    session_repo
        .upsert_session(&child)
        .await
        .expect("child session should be persisted");

    let now_ms = chrono::Utc::now().timestamp_millis();
    let governance = ScheduledTaskGovernanceSettings::default();

    let child_callback = ScheduledTaskService::create_scheduled_task_with_governance(
        &scheduled_repo,
        CreateScheduledTaskInput {
            name: "Child callback".to_string(),
            task_category: TASK_CATEGORY_SESSION.to_string(),
            cron_expression: None,
            schedule_timezone: "local".to_string(),
            assistant_id: "assistant-1".to_string(),
            group_id: None,
            group_name: None,
            message: "Child follow up".to_string(),
            yolo_mode: false,
            unsafe_mode: false,
            created_by_session_id: Some(child_id.to_string()),
            session_id: Some(child_id.to_string()),
            workspace_override: None,
            next_run_at: Some(now_ms + 60_000),
        },
        &governance,
    )
    .await
    .expect("child callback should be created");

    let deleted = ScheduledTaskService::delete_session_scheduled_tasks_for_sessions(
        &scheduled_repo,
        &[parent_id.to_string(), child_id.to_string()],
    )
    .await
    .expect("cascade cleanup should succeed");

    assert_eq!(deleted, 1);
    assert!(scheduled_repo
        .get_scheduled_task(&child_callback.id)
        .await
        .expect("lookup should succeed")
        .is_none());
}

#[tokio::test]
async fn scheduled_task_server_session_isolation_checks() {
    use tauri_mcp_agent_lib::mcp::builtin::scheduled_task::ScheduledTaskServer;
    use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
    use tauri_mcp_agent_lib::repositories::{
        SqliteAssistantRepository, SqliteScheduledTaskRepository, SqliteSessionRepository,
        SqliteSettingsRepository,
    };
    use tauri_mcp_agent_lib::{
        set_assistant_repository, set_scheduled_task_repository, set_session_repository,
        set_settings_repository,
    };

    let db = common::setup_test_db_with_migrations().await;

    // Register global repositories so the MCP handlers can retrieve them
    let session_repo = SqliteSessionRepository::new(db.clone());
    let scheduled_repo = SqliteScheduledTaskRepository::new(db.clone());
    let assistant_repo = SqliteAssistantRepository::new(db.clone());
    let settings_repo = SqliteSettingsRepository::new(db.clone());

    set_session_repository(session_repo.clone());
    set_scheduled_task_repository(SqliteScheduledTaskRepository::new(db.clone()));
    set_assistant_repository(assistant_repo.clone());
    set_settings_repository(settings_repo);

    // Create Session A and Session B
    let session_a_id = "session-a";
    let session_b_id = "session-b";

    session_repo
        .upsert_session(&make_session(session_a_id, "assistant-a"))
        .await
        .expect("session-a should be persisted");
    session_repo
        .upsert_session(&make_session(session_b_id, "assistant-b"))
        .await
        .expect("session-b should be persisted");

    common::seed_test_assistant(&db, "assistant-a", "Assistant A", serde_json::json!({})).await;
    common::seed_test_assistant(&db, "assistant-b", "Assistant B", serde_json::json!({})).await;

    // Create a SESSION task belonging to Session A
    let now_ms = chrono::Utc::now().timestamp_millis();
    let callback_task = ScheduledTaskService::create_scheduled_task(
        &scheduled_repo,
        CreateScheduledTaskInput {
            name: "Session A callback".to_string(),
            task_category: TASK_CATEGORY_SESSION.to_string(),
            cron_expression: None,
            schedule_timezone: "local".to_string(),
            assistant_id: "assistant-a".to_string(),
            group_id: None,
            group_name: None,
            message: "Session A message".to_string(),
            yolo_mode: false,
            unsafe_mode: false,
            created_by_session_id: Some(session_a_id.to_string()),
            session_id: Some(session_a_id.to_string()),
            workspace_override: None,
            next_run_at: Some(now_ms + 60_000),
        },
    )
    .await
    .expect("callback task for Session A should be created");

    // Initialize the ScheduledTaskServer as Session B
    let server_b =
        ScheduledTaskServer::new(session_b_id.to_string(), std::sync::Arc::new(db.clone()))
            .await
            .expect("server-b should initialize");

    // Test 1: getScheduledTask for Session A's callback by Session B should return permission denied
    let result = server_b
        .call_tool(
            "getScheduledTask",
            serde_json::json!({ "id": callback_task.id }),
            Some(session_b_id.to_string()),
        )
        .await
        .expect("call_tool should succeed");
    assert_eq!(result.is_error, Some(true));
    let error_text = extract_text_content(&result);
    assert!(error_text.contains("Permission denied"));
    assert!(error_text.contains("only manage session callbacks for your own session"));

    // Test 2: updateScheduledTask for Session A's callback by Session B should return permission denied
    let result = server_b
        .call_tool(
            "updateScheduledTask",
            serde_json::json!({ "id": callback_task.id, "name": "Hack" }),
            Some(session_b_id.to_string()),
        )
        .await
        .expect("call_tool should succeed");
    assert_eq!(result.is_error, Some(true));
    let error_text = extract_text_content(&result);
    assert!(error_text.contains("Permission denied"));

    // Test 3: toggleScheduledTask for Session A's callback by Session B should return permission denied
    let result = server_b
        .call_tool(
            "toggleScheduledTask",
            serde_json::json!({ "id": callback_task.id, "enabled": false }),
            Some(session_b_id.to_string()),
        )
        .await
        .expect("call_tool should succeed");
    assert_eq!(result.is_error, Some(true));
    let error_text = extract_text_content(&result);
    assert!(error_text.contains("Permission denied"));

    // Test 4: deleteScheduledTask for Session A's callback by Session B should return permission denied
    let result = server_b
        .call_tool(
            "deleteScheduledTask",
            serde_json::json!({ "id": callback_task.id }),
            Some(session_b_id.to_string()),
        )
        .await
        .expect("call_tool should succeed");
    assert_eq!(result.is_error, Some(true));
    let error_text = extract_text_content(&result);
    assert!(error_text.contains("Permission denied"));

    // Test 5: listScheduledTasks by Session B should NOT return Session A's callback
    let result = server_b
        .call_tool(
            "listScheduledTasks",
            serde_json::json!({}),
            Some(session_b_id.to_string()),
        )
        .await
        .expect("call_tool should succeed");
    assert_ne!(result.is_error, Some(true));
    let tasks_json = result
        .structured_content
        .as_ref()
        .unwrap()
        .get("tasks")
        .unwrap()
        .as_array()
        .unwrap();
    let has_session_a_callback = tasks_json
        .iter()
        .any(|t| t.get("id").unwrap().as_str().unwrap() == callback_task.id);
    assert!(!has_session_a_callback);

    // Test 6: get_service_context() on Session B's server should NOT contain Session A's callback
    let context = server_b.get_service_context(None).await;
    assert!(!context.context_prompt.contains(&callback_task.id));
    assert!(!context.context_prompt.contains("Session A callback"));
}
