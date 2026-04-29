mod common;

use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::session::{
    ensure_session_workspace_dir, hydrate_persisted_workspace_override,
    hydrate_persisted_workspace_override_from_global, resolve_session_workspace_dir,
    SessionManager,
};
use tauri_mcp_agent_lib::set_session_repository;
use tokio::sync::OnceCell;

static TEST_DB: OnceCell<sea_orm::DatabaseConnection> = OnceCell::const_new();

async fn test_db() -> sea_orm::DatabaseConnection {
    TEST_DB
        .get_or_init(|| async {
            let db = common::setup_test_db_with_migrations().await;
            set_session_repository(SqliteSessionRepository::new(db.clone()));
            db
        })
        .await
        .clone()
}

fn make_session(session_id: &str, workspace_override: Option<String>) -> SessionMetadata {
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Test Session".to_string()),
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
        workspace_override,
    }
}

#[tokio::test]
async fn hydrates_persisted_workspace_override_into_session_manager() {
    let db = test_db().await;
    let repo = SqliteSessionRepository::new(db);
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let override_dir = temp_dir.path().join("override");
    tokio::fs::create_dir_all(&override_dir)
        .await
        .expect("override dir should be created");

    let session_id = "hydrate-session";
    repo.upsert_session(&make_session(
        session_id,
        Some(override_dir.to_string_lossy().to_string()),
    ))
    .await
    .expect("session should be persisted");

    let session_manager =
        SessionManager::new_with_base_dir(temp_dir.path().join("session-root")).unwrap();

    let hydrated = hydrate_persisted_workspace_override(&repo, &session_manager, session_id)
        .await
        .expect("hydrate should succeed");

    assert_eq!(hydrated.as_deref(), Some(override_dir.as_path()));

    let session_info = session_manager
        .get_session_info(session_id)
        .expect("session info should exist after hydration");
    assert_eq!(
        session_info.workspace_override.as_deref(),
        Some(override_dir.as_path())
    );
}

#[tokio::test]
async fn ensure_session_workspace_dir_repairs_poisoned_default_workspace_entry() {
    let db = test_db().await;
    let repo = SqliteSessionRepository::new(db);
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let override_dir = temp_dir.path().join("real-workspace");
    tokio::fs::create_dir_all(&override_dir)
        .await
        .expect("override dir should be created");

    let session_id = "poisoned-session";
    repo.upsert_session(&make_session(
        session_id,
        Some(override_dir.to_string_lossy().to_string()),
    ))
    .await
    .expect("session should be persisted");

    let session_manager =
        SessionManager::new_with_base_dir(temp_dir.path().join("session-root")).unwrap();

    let default_workspace = session_manager.get_session_workspace_dir_by_id(session_id);
    assert_ne!(default_workspace, override_dir);
    assert_eq!(
        session_manager
            .get_session_info(session_id)
            .expect("poisoned entry should exist")
            .workspace_override,
        None
    );

    let repaired_workspace = ensure_session_workspace_dir(&repo, &session_manager, session_id)
        .await
        .expect("workspace resolution should succeed");

    assert_eq!(repaired_workspace, override_dir);
    assert_eq!(
        session_manager
            .get_session_info(session_id)
            .expect("session info should still exist")
            .workspace_override
            .as_deref(),
        Some(override_dir.as_path())
    );
}

#[tokio::test]
async fn resolve_session_workspace_dir_repairs_poisoned_entry_via_global_repo() {
    let db = test_db().await;
    let repo = SqliteSessionRepository::new(db);
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let override_dir = temp_dir.path().join("global-real-workspace");
    tokio::fs::create_dir_all(&override_dir)
        .await
        .expect("override dir should be created");

    let session_id = "global-poisoned-session";
    repo.upsert_session(&make_session(
        session_id,
        Some(override_dir.to_string_lossy().to_string()),
    ))
    .await
    .expect("session should be persisted");

    let session_manager =
        SessionManager::new_with_base_dir(temp_dir.path().join("session-root")).unwrap();
    let poisoned_workspace = session_manager.get_session_workspace_dir_by_id(session_id);
    assert_ne!(poisoned_workspace, override_dir);

    let resolved_workspace = resolve_session_workspace_dir(&session_manager, session_id)
        .await
        .expect("global repo resolution should succeed");

    assert_eq!(resolved_workspace, override_dir);
    assert_eq!(
        session_manager
            .get_session_info(session_id)
            .expect("session info should exist after global repair")
            .workspace_override
            .as_deref(),
        Some(override_dir.as_path())
    );
}

#[tokio::test]
async fn global_hydration_repairs_poisoned_entry_without_reading_workspace_first() {
    let db = test_db().await;
    let repo = SqliteSessionRepository::new(db);
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let override_dir = temp_dir.path().join("open-session-workspace");
    tokio::fs::create_dir_all(&override_dir)
        .await
        .expect("override dir should be created");

    let session_id = "open-session-hydration";
    repo.upsert_session(&make_session(
        session_id,
        Some(override_dir.to_string_lossy().to_string()),
    ))
    .await
    .expect("session should be persisted");

    let session_manager =
        SessionManager::new_with_base_dir(temp_dir.path().join("session-root")).unwrap();
    let _ = session_manager.get_session_workspace_dir_by_id(session_id);

    hydrate_persisted_workspace_override_from_global(&session_manager, session_id)
        .await
        .expect("global hydration should succeed");

    assert_eq!(
        session_manager.get_session_workspace_dir_by_id(session_id),
        override_dir
    );
}
