//! Persistence query and connection-pragma coverage for issue #2027.
//!
//! Standalone binary so it runs on Windows CI. `integration_tests` is
//! `#![cfg(not(windows))]` because it links WebView.

use std::collections::HashSet;

use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement};
use sea_orm_migration::MigratorTrait;
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::models::chat::Message;
use tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode;
use tauri_mcp_agent_lib::repositories::{
    MessageRepository, SessionMetadata, SessionRepository, SessionStatus, SqliteMessageRepository,
    SqliteSessionRepository,
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
        name: Some("Persistence query session".to_string()),
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

fn build_message(session_id: &str, id: &str, created_at: i64, text: &str) -> Message {
    Message {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: "user".to_string(),
        content: vec![MCPContent::Text {
            text: text.to_string(),
        }],
        tool_calls: None,
        tool_call_id: None,
        is_streaming: Some(false),
        thinking: Some("secret thinking block".to_string()),
        thinking_signature: None,
        assistant_id: None,
        attachments: Some(serde_json::json!([{ "name": "secret-attachment.bin" }])),
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

async fn index_names(db: &DatabaseConnection) -> HashSet<String> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'index'".to_string(),
        ))
        .await
        .expect("index list should query");
    rows.into_iter()
        .map(|row| {
            row.try_get::<String>("", "name")
                .expect("index name should be text")
        })
        .collect()
}

#[tokio::test]
async fn migration_creates_planning_graph_and_schedule_indexes() {
    let db = setup_isolated_db().await;
    let names = index_names(&db).await;
    for expected in [
        "idx-planning_todos-session_created_id",
        "idx-planning_goals-session_status",
        "idx-knowledge_relationships-assistant_source",
        "idx-knowledge_relationships-assistant_target",
        "idx-scheduled_tasks-enabled_next_run",
    ] {
        assert!(names.contains(expected), "missing index {expected}");
    }
}

#[tokio::test]
async fn dirty_sessions_are_one_query_and_match_index_meta() {
    let (session_repo, message_repo) = setup_repos().await;

    session_repo
        .upsert_session(&build_session_metadata("empty-session"))
        .await
        .expect("empty session");
    session_repo
        .upsert_session(&build_session_metadata("never-indexed"))
        .await
        .expect("never-indexed session");
    session_repo
        .upsert_session(&build_session_metadata("fresh-index"))
        .await
        .expect("fresh-index session");
    session_repo
        .upsert_session(&build_session_metadata("stale-index"))
        .await
        .expect("stale-index session");

    message_repo
        .insert(&build_message("never-indexed", "m-new", 1_000, "new"))
        .await
        .expect("insert never-indexed");
    message_repo
        .insert(&build_message("fresh-index", "m-fresh", 1_000, "fresh"))
        .await
        .expect("insert fresh");
    message_repo
        .update_index_meta("fresh-index", "/tmp/fresh.idx", 1, 1)
        .await
        .expect("mark fresh index");
    message_repo
        .insert(&build_message("stale-index", "m-old", 1_000, "old"))
        .await
        .expect("insert stale base");
    message_repo
        .update_index_meta("stale-index", "/tmp/stale.idx", 1, 1)
        .await
        .expect("mark stale index");
    let newer_than_index = chrono::Utc::now().timestamp_millis() + 60_000;
    message_repo
        .insert(&build_message(
            "stale-index",
            "m-newer",
            newer_than_index,
            "newer",
        ))
        .await
        .expect("insert message after index");

    let dirty: HashSet<String> = message_repo
        .get_dirty_session_ids()
        .await
        .expect("dirty session query")
        .into_iter()
        .collect();

    assert_eq!(
        dirty,
        HashSet::from(["never-indexed".to_string(), "stale-index".to_string()])
    );
    assert!(!message_repo
        .is_index_dirty("fresh-index")
        .await
        .expect("fresh dirty check"));
    assert!(message_repo
        .is_index_dirty("stale-index")
        .await
        .expect("stale dirty check"));
}

#[tokio::test]
async fn index_projection_skips_thinking_and_keeps_newest_row() {
    let (session_repo, message_repo) = setup_repos().await;
    session_repo
        .upsert_session(&build_session_metadata("indexed"))
        .await
        .expect("session");

    message_repo
        .insert(&build_message("indexed", "older", 1, "older body"))
        .await
        .expect("older");
    message_repo
        .insert(&build_message("indexed", "newer", 2, "indexed body"))
        .await
        .expect("newer");

    let docs = message_repo
        .get_index_documents_by_session("indexed", 1)
        .await
        .expect("index documents");
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, "newer");
    assert!(docs[0].content.contains("indexed body"));
    assert!(!docs[0].content.contains("secret thinking block"));
    assert!(!docs[0].content.contains("secret-attachment.bin"));
}

#[tokio::test]
async fn insert_many_batches_and_keeps_latest_session_timestamp() {
    let (session_repo, message_repo) = setup_repos().await;
    session_repo
        .upsert_session(&build_session_metadata("batched"))
        .await
        .expect("session");

    message_repo
        .insert_many(Vec::new())
        .await
        .expect("empty insert_many");

    let messages: Vec<Message> = (1..=70)
        .map(|n| build_message("batched", &format!("batch-{n}"), n, "body"))
        .collect();
    message_repo
        .insert_many(messages)
        .await
        .expect("batched insert");

    let stored = message_repo
        .get_messages_by_session("batched", 100)
        .await
        .expect("reload");
    assert_eq!(stored.len(), 70);

    let session = session_repo
        .get_session("batched")
        .await
        .expect("session read")
        .expect("session exists");
    assert_eq!(session.last_message_at, Some(70));
}

#[tokio::test]
async fn app_sqlite_connection_uses_wal_normal_sync_and_memory_cache() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("pragma.db");
    let path = path.to_str().expect("utf8 path");
    let db = tauri_mcp_agent_lib::lifecycle::database::connect_sqlite_with_app_pragmas(path, true)
        .await
        .expect("configured sqlite connection");

    let sync = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA synchronous".to_string(),
        ))
        .await
        .expect("synchronous pragma")
        .expect("synchronous row");
    let level: i64 = sync.try_get("", "synchronous").expect("synchronous value");
    assert_eq!(level, 1, "NORMAL is 1");

    let cache = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA cache_size".to_string(),
        ))
        .await
        .expect("cache pragma")
        .expect("cache row");
    let cache_size: i64 = cache.try_get("", "cache_size").expect("cache_size value");
    assert_eq!(cache_size, -32_768);

    let temp_store = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA temp_store".to_string(),
        ))
        .await
        .expect("temp_store pragma")
        .expect("temp_store row");
    let temp_store: i64 = temp_store
        .try_get("", "temp_store")
        .expect("temp_store value");
    assert_eq!(temp_store, 2, "MEMORY is 2");

    let journal = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA journal_mode".to_string(),
        ))
        .await
        .expect("journal pragma")
        .expect("journal row");
    let mode: String = journal
        .try_get("", "journal_mode")
        .expect("journal_mode value");
    assert_eq!(mode.to_ascii_lowercase(), "wal");
}
