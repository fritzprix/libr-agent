mod common;

use tauri_mcp_agent_lib::repositories::session_repository::SessionAttentionReason;
use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};

async fn setup_repo() -> SqliteSessionRepository {
    let db = common::setup_test_db_with_migrations().await;
    SqliteSessionRepository::new(db)
}

fn build_session(id: &str) -> SessionMetadata {
    SessionMetadata {
        id: id.to_string(),
        name: Some("Attention Session".to_string()),
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
async fn mark_viewed_clears_acknowledged_attention() {
    let repo = setup_repo().await;
    let session = build_session("session-1");
    repo.upsert_session(&session)
        .await
        .expect("session should insert");

    repo.update_attention("session-1", 2_000, SessionAttentionReason::PendingApproval)
        .await
        .expect("attention should persist");
    repo.update_last_viewed_at("session-1", 2_000)
        .await
        .expect("viewed timestamp should persist");

    let updated = repo
        .get_session("session-1")
        .await
        .expect("session lookup should succeed")
        .expect("session should exist");

    assert_eq!(updated.last_viewed_at, Some(2_000));
    assert_eq!(updated.last_attention_at, None);
    assert_eq!(updated.last_attention_reason, None);
}

#[tokio::test]
async fn mark_viewed_keeps_newer_attention_unread() {
    let repo = setup_repo().await;
    let session = build_session("session-2");
    repo.upsert_session(&session)
        .await
        .expect("session should insert");

    repo.update_attention("session-2", 3_000, SessionAttentionReason::RecurringStop)
        .await
        .expect("attention should persist");
    repo.update_last_viewed_at("session-2", 2_000)
        .await
        .expect("viewed timestamp should persist");

    let updated = repo
        .get_session("session-2")
        .await
        .expect("session lookup should succeed")
        .expect("session should exist");

    assert_eq!(updated.last_viewed_at, Some(2_000));
    assert_eq!(updated.last_attention_at, Some(3_000));
    assert_eq!(
        updated.last_attention_reason,
        Some(SessionAttentionReason::RecurringStop)
    );
}
