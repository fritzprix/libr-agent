mod common;

use sea_orm::{ActiveModelTrait, Set};
use std::str::FromStr;
use tauri_mcp_agent_lib::entity::session;
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
        model: "gpt-4".to_string(),
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
async fn create_and_get_session() {
    let repo = setup_repo().await;
    let mut session = build_session("test-session-1", 1_000);
    session.name = Some("Test Session".to_string());
    session.agent_config = Some(r#"{"model":"gpt-4"}"#.to_string());

    repo.upsert_session(&session)
        .await
        .expect("session should insert");

    let retrieved = repo
        .get_session("test-session-1")
        .await
        .expect("session lookup should succeed")
        .expect("session should exist");

    assert_eq!(retrieved.id, "test-session-1");
    assert_eq!(retrieved.name.as_deref(), Some("Test Session"));
    assert_eq!(retrieved.status, SessionStatus::Idle);
}

#[tokio::test]
async fn update_status_persists_new_timestamp() {
    let repo = setup_repo().await;
    let session = build_session("test-session-2", 2_000);

    repo.upsert_session(&session)
        .await
        .expect("session should insert");

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    repo.update_status("test-session-2", SessionStatus::Busy)
        .await
        .expect("status update should succeed");

    let retrieved = repo
        .get_session("test-session-2")
        .await
        .expect("session lookup should succeed")
        .expect("session should exist");

    assert_eq!(retrieved.status, SessionStatus::Busy);
    assert!(retrieved.updated_at > session.updated_at);
}

#[tokio::test]
async fn get_all_sessions_returns_descending_updated_at_order() {
    let repo = setup_repo().await;

    for (id, updated_at) in [
        ("test-session-1", 1_000),
        ("test-session-2", 2_000),
        ("test-session-3", 3_000),
    ] {
        repo.upsert_session(&build_session(id, updated_at))
            .await
            .expect("session should insert");
    }

    let sessions = repo
        .get_all_sessions()
        .await
        .expect("all sessions should load");

    assert_eq!(
        sessions
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>(),
        vec!["test-session-3", "test-session-2", "test-session-1"]
    );
}

#[test]
fn session_status_serialization_round_trips() {
    assert_eq!(SessionStatus::Idle.as_str(), "idle");
    assert_eq!(SessionStatus::Busy.as_str(), "busy");
    assert_eq!(SessionStatus::Paused.as_str(), "paused");
    assert_eq!(SessionStatus::Error.as_str(), "error");

    assert_eq!(
        SessionStatus::from_str("idle").unwrap(),
        SessionStatus::Idle
    );
    assert_eq!(
        SessionStatus::from_str("busy").unwrap(),
        SessionStatus::Busy
    );
    assert_eq!(
        SessionStatus::from_str("paused").unwrap(),
        SessionStatus::Paused
    );
    assert_eq!(
        SessionStatus::from_str("error").unwrap(),
        SessionStatus::Error
    );
    assert!(SessionStatus::from_str("invalid").is_err());
}

#[tokio::test]
async fn upsert_session_updates_existing_rows() {
    let repo = setup_repo().await;
    let original = build_session("test-session-update", 1_000);

    repo.upsert_session(&original)
        .await
        .expect("original session should insert");

    let mut updated = build_session("test-session-update", 2_000);
    updated.name = Some("Updated Name".to_string());
    updated.status = SessionStatus::Busy;
    updated.agent_config = Some(r#"{"updated":true}"#.to_string());

    repo.upsert_session(&updated)
        .await
        .expect("session should upsert");

    let retrieved = repo
        .get_session("test-session-update")
        .await
        .expect("session lookup should succeed")
        .expect("session should exist");

    assert_eq!(retrieved.name.as_deref(), Some("Updated Name"));
    assert_eq!(retrieved.status, SessionStatus::Busy);
    assert_eq!(
        retrieved.agent_config,
        Some(r#"{"updated":true}"#.to_string())
    );
}

#[tokio::test]
async fn delete_session_removes_persisted_row() {
    let repo = setup_repo().await;
    let session = build_session("test-session-delete", 1_000);

    repo.upsert_session(&session)
        .await
        .expect("session should insert");
    assert!(repo
        .get_session("test-session-delete")
        .await
        .expect("lookup should succeed")
        .is_some());

    repo.delete_session("test-session-delete")
        .await
        .expect("delete should succeed");

    assert!(repo
        .get_session("test-session-delete")
        .await
        .expect("lookup should succeed")
        .is_none());
}

#[tokio::test]
async fn toggle_bookmark_persists_boolean_value() {
    let repo = setup_repo().await;
    let session = build_session("test-bookmark", 1_000);

    repo.upsert_session(&session)
        .await
        .expect("session should insert");

    repo.toggle_bookmark("test-bookmark", true)
        .await
        .expect("bookmark should persist");
    assert!(
        repo.get_session("test-bookmark")
            .await
            .expect("lookup should succeed")
            .expect("session should exist")
            .is_bookmarked
    );

    repo.toggle_bookmark("test-bookmark", false)
        .await
        .expect("bookmark clear should persist");
    assert!(
        !repo
            .get_session("test-bookmark")
            .await
            .expect("lookup should succeed")
            .expect("session should exist")
            .is_bookmarked
    );
}

#[tokio::test]
async fn update_name_persists_title_without_touching_updated_at() {
    let repo = setup_repo().await;
    let session = build_session("test-rename", 42_000);

    repo.upsert_session(&session)
        .await
        .expect("session should insert");

    repo.update_name("test-rename", "Renamed Session".to_string())
        .await
        .expect("title update should persist");

    let retrieved = repo
        .get_session("test-rename")
        .await
        .expect("lookup should succeed")
        .expect("session should exist");

    assert_eq!(retrieved.name.as_deref(), Some("Renamed Session"));
    assert_eq!(retrieved.updated_at, session.updated_at);
}

#[tokio::test]
async fn get_session_coalesces_legacy_execution_flags() {
    let db = common::setup_test_db_with_migrations().await;
    let repo = SqliteSessionRepository::new(db.clone());

    session::ActiveModel {
        id: Set("legacy-session".to_string()),
        name: Set(Some("Legacy Session".to_string())),
        status: Set("idle".to_string()),
        model: Set("gpt-4".to_string()),
        provider: Set("openai".to_string()),
        agent_config: Set(None),
        parent_session_id: Set(None),
        lineage_id: Set(None),
        depth: Set(None),
        max_depth: Set(None),
        max_fanout: Set(None),
        org_id: Set(None),
        org_name: Set(None),
        org_root_session_id: Set(None),
        created_at: Set(1_000),
        updated_at: Set(1_000),
        last_viewed_at: Set(None),
        last_message_at: Set(None),
        last_attention_at: Set(None),
        last_attention_reason: Set(None),
        is_bookmarked: Set(false),
        yolo_mode: Set(true),
        unsafe_mode: Set(true),
        workspace_override: Set(None),
    }
    .insert(&db)
    .await
    .expect("legacy row should insert");

    let retrieved = repo
        .get_session("legacy-session")
        .await
        .expect("session lookup should succeed")
        .expect("session should exist");

    assert!(!retrieved.yolo_mode);
    assert!(retrieved.unsafe_mode);
}
