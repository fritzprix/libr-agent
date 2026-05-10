mod common;

use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::repositories::{
    DbError, MessageRepository, SessionMetadata, SessionRepository, SessionStatus,
    SqliteMessageRepository, SqliteSessionRepository,
};

fn build_session_metadata(session_id: &str) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Message pagination regression".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-5.4".to_string(),
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
        created_at: now,
        updated_at: now,
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
        created_at,
        updated_at: created_at,
        source: None,
        error: None,
        metadata: None,
    }
}

#[tokio::test]
async fn message_history_pagination_uses_rowid_for_same_timestamp_ties() {
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let message_repo = SqliteMessageRepository::new(db.clone());
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
        .get_messages_before(
            &session_id,
            oldest_cursor.created_at,
            oldest_cursor.row_id,
            2,
        )
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
async fn message_slice_queries_reject_zero_limit() {
    let db = common::setup_test_db_with_migrations().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let message_repo = SqliteMessageRepository::new(db.clone());
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
        .get_messages_before(&session_id, 1_712_345_678_900_i64, i64::MAX, 0)
        .await
        .expect_err("zero limit should be rejected for older slices");
    assert!(matches!(before_error, DbError::InvalidInput(_)));
}
