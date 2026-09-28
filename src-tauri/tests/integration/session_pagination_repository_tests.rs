use crate::common;

use tauri_mcp_agent_lib::agent::ExecutionMode;
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
        assistant_id: None,
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
        execution_mode: ExecutionMode::Normal,
        workspace_override: None,
        workspace_isolation:
            tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
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
        .list_sessions(None, 2, None, false, None)
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
        .list_sessions(first_page.next_cursor.clone(), 2, None, false, None)
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
async fn list_sessions_filters_by_search_across_pages() {
    let repo = setup_repo().await;
    for (id, name, updated_at) in [
        ("keep-old", "Alpha Project", 1_000),
        ("noise-mid", "Unrelated", 2_000),
        ("keep-new", "alpha notes", 3_000),
        ("id-hit", "Something Else", 4_000),
    ] {
        let mut session = build_session(id, updated_at);
        session.name = Some(name.to_string());
        repo.upsert_session(&session)
            .await
            .expect("session should insert");
    }

    let first_page = repo
        .list_sessions(None, 2, Some("alpha"), false, None)
        .await
        .expect("search page should load");

    assert_eq!(
        first_page
            .items
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>(),
        vec!["keep-new", "keep-old"]
    );
    assert!(first_page.next_cursor.is_none());

    let id_match = repo
        .list_sessions(None, 10, Some("id-hit"), false, None)
        .await
        .expect("id search should load");
    assert_eq!(
        id_match
            .items
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>(),
        vec!["id-hit"]
    );
}

#[tokio::test]
async fn list_sessions_search_treats_like_wildcards_as_literals() {
    let repo = setup_repo().await;
    let mut underscored = build_session("s-under", 2_000);
    underscored.name = Some("foo_bar".to_string());
    let mut plain = build_session("s-plain", 1_000);
    plain.name = Some("foobar".to_string());

    repo.upsert_session(&underscored)
        .await
        .expect("underscored session should insert");
    repo.upsert_session(&plain)
        .await
        .expect("plain session should insert");

    let page = repo
        .list_sessions(None, 10, Some("foo_bar"), false, None)
        .await
        .expect("literal underscore search should load");

    assert_eq!(
        page.items
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>(),
        vec!["s-under"]
    );
}

#[tokio::test]
async fn list_sessions_filters_bookmarked_and_status_before_limit() {
    let repo = setup_repo().await;
    for (id, name, updated_at, bookmarked, status) in [
        ("bm-busy", "Alpha Busy", 5_000, true, SessionStatus::Busy),
        ("bm-idle", "Alpha Idle", 4_000, true, SessionStatus::Idle),
        (
            "plain-busy",
            "Alpha Plain",
            3_000,
            false,
            SessionStatus::Busy,
        ),
        ("bm-other", "Beta", 2_000, true, SessionStatus::Busy),
    ] {
        let mut session = build_session(id, updated_at);
        session.name = Some(name.to_string());
        session.is_bookmarked = bookmarked;
        session.status = status;
        repo.upsert_session(&session)
            .await
            .expect("session should insert");
    }

    let bookmarked_search = repo
        .list_sessions(None, 10, Some("alpha"), true, None)
        .await
        .expect("bookmarked search should load");
    assert_eq!(
        bookmarked_search
            .items
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>(),
        vec!["bm-busy", "bm-idle"]
    );

    let busy_search = repo
        .list_sessions(None, 10, Some("alpha"), false, Some("busy"))
        .await
        .expect("status search should load");
    assert_eq!(
        busy_search
            .items
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>(),
        vec!["bm-busy", "plain-busy"]
    );

    let combined = repo
        .list_sessions(None, 10, Some("alpha"), true, Some("busy"))
        .await
        .expect("combined filters should load");
    assert_eq!(
        combined
            .items
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>(),
        vec!["bm-busy"]
    );
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
