//! Windows-safe coverage for message repository slice pagination and
//! error-column structuredContent envelope persistence.
//!
//! Standalone binary — does **not** pull `tauri::test::mock_app` / WebView into the
//! link (those live in consolidated `tests/integration_tests.rs` and crash on
//! Windows with STATUS_ENTRYPOINT_NOT_FOUND). Also avoids `reset_state()` /
//! global AppHandle OnceLocks.

use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement};
use sea_orm_migration::MigratorTrait;
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::repositories::{
    DbError, MessageRepository, SessionMetadata, SessionRepository, SessionStatus,
    SqliteMessageRepository, SqliteSessionRepository,
};

async fn setup_isolated_db() -> DatabaseConnection {
    tauri_mcp_agent_lib::lifecycle::database::register_sqlite_vec();
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite should connect");
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "PRAGMA foreign_keys = ON".to_string(),
    ))
    .await
    .expect("foreign_keys pragma should apply");
    Migrator::up(&db, None)
        .await
        .expect("migrations should run");
    db
}

fn build_session_metadata(session_id: &str) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Message pagination regression".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        assistant_id: None,
        parent_session_id: None,
        lineage_id: None,
        depth: None,
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: now,
        updated_at: now,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation: WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
    }
}

fn build_message(session_id: &str, id: &str, created_at: i64) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: "assistant".to_string(),
        content: vec![],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        usage: None,
        prompt_tokens: None,
        created_at,
        updated_at: created_at,
        source: None,
        error: None,
        metadata: None,
    }
}

async fn setup_repos() -> (SqliteSessionRepository, SqliteMessageRepository) {
    let db = setup_isolated_db().await;
    (
        SqliteSessionRepository::new(db.clone()),
        SqliteMessageRepository::new(db),
    )
}

#[tokio::test]
async fn message_history_pagination_uses_rowid_for_same_timestamp_ties() {
    let (session_repo, message_repo) = setup_repos().await;
    let session_id = format!("pagination-{}", uuid::Uuid::new_v4());

    session_repo
        .upsert_session(&build_session_metadata(&session_id))
        .await
        .expect("session should be created");

    let created_at = 1_712_345_678_900_i64;
    let inserted_ids = ["msg-z", "msg-a", "msg-m", "msg-b"];
    for id in inserted_ids {
        message_repo
            .insert(&build_message(&session_id, id, created_at))
            .await
            .expect("message insert should succeed");
    }

    let first_slice = message_repo
        .get_recent_slice(&session_id, 2)
        .await
        .expect("recent slice should load");

    let first_ids: Vec<String> = first_slice
        .items
        .iter()
        .map(|message| message.id.clone())
        .collect();
    assert_eq!(first_ids, vec!["msg-m".to_string(), "msg-b".to_string()]);
    assert!(first_slice.has_more_before);

    let oldest_cursor = first_slice
        .oldest_cursor
        .clone()
        .expect("recent slice should expose oldest cursor");

    let older_slice = message_repo
        .get_messages_before(&session_id, oldest_cursor.row_id, 2)
        .await
        .expect("older slice should load");

    let older_ids: Vec<String> = older_slice
        .items
        .iter()
        .map(|message| message.id.clone())
        .collect();
    assert_eq!(older_ids, vec!["msg-z".to_string(), "msg-a".to_string()]);
    assert!(!older_slice.has_more_before);
}

#[tokio::test]
async fn message_history_pagination_prefers_rowid_over_inverted_created_at() {
    let (session_repo, message_repo) = setup_repos().await;
    let session_id = format!("pagination-inverted-{}", uuid::Uuid::new_v4());

    session_repo
        .upsert_session(&build_session_metadata(&session_id))
        .await
        .expect("session should be created");

    message_repo
        .insert(&build_message(&session_id, "assistant-owner", 2_000))
        .await
        .expect("assistant owner insert should succeed");
    message_repo
        .insert(&build_message(&session_id, "tool-result-a", 1_000))
        .await
        .expect("tool result A insert should succeed");
    message_repo
        .insert(&build_message(&session_id, "tool-result-b", 1_001))
        .await
        .expect("tool result B insert should succeed");

    let first_slice = message_repo
        .get_recent_slice(&session_id, 2)
        .await
        .expect("recent slice should load");

    let first_ids: Vec<String> = first_slice
        .items
        .iter()
        .map(|message| message.id.clone())
        .collect();
    assert_eq!(
        first_ids,
        vec!["tool-result-a".to_string(), "tool-result-b".to_string()]
    );

    let oldest_cursor = first_slice
        .oldest_cursor
        .clone()
        .expect("recent slice should expose oldest cursor");

    let older_slice = message_repo
        .get_messages_before(&session_id, oldest_cursor.row_id, 2)
        .await
        .expect("older slice should load");

    let older_ids: Vec<String> = older_slice
        .items
        .iter()
        .map(|message| message.id.clone())
        .collect();
    assert_eq!(older_ids, vec!["assistant-owner".to_string()]);
    assert!(!older_slice.has_more_before);
}

#[tokio::test]
async fn message_slice_queries_reject_zero_limit() {
    let (session_repo, message_repo) = setup_repos().await;
    let session_id = format!("pagination-zero-{}", uuid::Uuid::new_v4());

    session_repo
        .upsert_session(&build_session_metadata(&session_id))
        .await
        .expect("session should be created");
    message_repo
        .insert(&build_message(&session_id, "msg-1", 1_712_345_678_900_i64))
        .await
        .expect("message insert should succeed");

    let recent_error = message_repo
        .get_recent_slice(&session_id, 0)
        .await
        .expect_err("zero limit should be rejected for recent slices");
    assert!(matches!(recent_error, DbError::InvalidInput(_)));

    let before_error = message_repo
        .get_messages_before(&session_id, i64::MAX, 0)
        .await
        .expect_err("zero limit should be rejected for older slices");
    assert!(matches!(before_error, DbError::InvalidInput(_)));
}

#[tokio::test]
async fn reload_preserves_structured_content_in_error_column_envelope() {
    let (session_repo, message_repo) = setup_repos().await;
    let session_id = format!("structured-{}", uuid::Uuid::new_v4());
    session_repo
        .upsert_session(&build_session_metadata(&session_id))
        .await
        .expect("session should be created");

    let structured = serde_json::json!({
        "sessionId": "a1b2c3d4e5",
        "status": "started",
        "responseStatus": "pending",
        "toolName": "startSession",
    });

    let mut message = build_message(&session_id, "tool-structured", 1_000);
    message.role = "tool".to_string();
    message.tool_call_id = Some("call-spawn".to_string());
    message.content = vec![MCPContent::Text {
        text: "Session started successfully".to_string(),
    }];
    message.metadata = Some(serde_json::json!({
        "structuredContent": structured,
    }));

    message_repo
        .insert(&message)
        .await
        .expect("insert should succeed");

    let loaded = message_repo
        .get_by_id("tool-structured")
        .await
        .expect("lookup should succeed")
        .expect("message should exist");

    assert_eq!(
        loaded
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("structuredContent")),
        Some(&structured),
        "structuredContent must survive DB reload for structured tool cards"
    );
    assert!(
        loaded.error.is_none(),
        "structuredContent envelope must not surface as Message.error"
    );
}

#[tokio::test]
async fn reload_preserves_tool_error_and_structured_content_together() {
    let (session_repo, message_repo) = setup_repos().await;
    let session_id = format!("structured-err-{}", uuid::Uuid::new_v4());
    session_repo
        .upsert_session(&build_session_metadata(&session_id))
        .await
        .expect("session should be created");

    let structured = serde_json::json!({ "path": "a.txt", "action": "created" });
    let mut message = build_message(&session_id, "tool-both", 1_000);
    message.role = "tool".to_string();
    message.tool_call_id = Some("call-both".to_string());
    message.content = vec![MCPContent::Text {
        text: "write failed".to_string(),
    }];
    message.metadata = Some(serde_json::json!({
        "toolError": true,
        "structuredContent": structured,
    }));

    message_repo
        .insert(&message)
        .await
        .expect("insert should succeed");

    let loaded = message_repo
        .get_by_id("tool-both")
        .await
        .expect("lookup should succeed")
        .expect("message should exist");

    assert_eq!(
        loaded
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("toolError"))
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        loaded
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("structuredContent")),
        Some(&structured)
    );
    assert!(loaded.error.is_none());
}
