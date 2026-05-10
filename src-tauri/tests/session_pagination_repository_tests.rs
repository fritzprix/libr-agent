mod common;

use tauri_mcp_agent_lib::repositories::session_repository::SessionAttentionReason;
use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};

async fn setup_repo() -> SqliteSessionRepository {
    let db = common::setup_test_db_with_migrations().await;
    SqliteSessionRepository::new(db)
}

fn build_session(id: &str, updated_at: i64) -> SessionMetadata {
    SessionMetadata {
        id: id.to_string(),
        name: Some(format!("Session {id}")),
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
        created_at: updated_at,
        updated_at,
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
async fn list_sessions_returns_cursor_ordered_pages() {
    let repo = setup_repo().await;
    for (id, updated_at) in [
        ("session-a", 1_000),
        ("session-b", 3_000),
        ("session-c", 2_000),
    ] {
        repo.upsert_session(&build_session(id, updated_at))
            .await
            .expect("session should insert");
    }

    let first_page = repo
        .list_sessions(None, 2)
        .await
        .expect("first page should load");

    assert_eq!(
        first_page
            .items
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>(),
        vec!["session-b", "session-c"]
    );

    let second_page = repo
        .list_sessions(first_page.next_cursor.clone(), 2)
        .await
        .expect("second page should load");

    assert_eq!(
        second_page
            .items
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>(),
        vec!["session-a"]
    );
    assert!(second_page.next_cursor.is_none());
}

#[tokio::test]
async fn list_attention_sessions_only_returns_unread_attention() {
    let repo = setup_repo().await;
    let unread = build_session("unread", 3_000);
    let read = build_session("read", 2_000);
    let idle = build_session("idle", 1_000);

    repo.upsert_session(&unread)
        .await
        .expect("unread session should insert");
    repo.upsert_session(&read)
        .await
        .expect("read session should insert");
    repo.upsert_session(&idle)
        .await
        .expect("idle session should insert");

    repo.update_attention("unread", 4_000, SessionAttentionReason::PendingApproval)
        .await
        .expect("unread attention should persist");
    repo.update_attention("read", 3_500, SessionAttentionReason::RecurringStop)
        .await
        .expect("read attention should persist");
    repo.update_last_viewed_at("read", 3_500)
        .await
        .expect("read viewed state should persist");

    let attention_sessions = repo
        .list_attention_sessions()
        .await
        .expect("attention sessions should load");

    assert_eq!(
        attention_sessions
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>(),
        vec!["unread"]
    );
}
