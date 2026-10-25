use crate::common;

use migration::MigratorTrait;
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use tauri_mcp_agent_lib::agent::session_manager::{
    dump_raw_compaction_output_if_needed, CompactSummaryClampResult,
};
use tauri_mcp_agent_lib::agent::ExecutionMode;
use tauri_mcp_agent_lib::repositories::{
    SessionMetadata, SessionRepository, SessionStatus, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::services::workspace_service::WorkspaceService;
use tauri_mcp_agent_lib::set_session_repository;
use tauri_mcp_agent_lib::utils::sqlite::format_sqlite_url;
use tempfile::TempDir;
use tokio::sync::{Mutex, MutexGuard, OnceCell};

struct TestContext {
    _temp_dir: TempDir,
    db: sea_orm::DatabaseConnection,
}

static TEST_CONTEXT: OnceCell<TestContext> = OnceCell::const_new();
static TEST_MUTEX: Mutex<()> = Mutex::const_new(());

async fn test_db() -> sea_orm::DatabaseConnection {
    TEST_CONTEXT
        .get_or_init(|| async {
            common::register_sqlite_vec();
            let temp_dir = tempfile::tempdir().expect("temp dir should be created");
            let db_path = temp_dir.path().join("compaction-raw-dump-tests.db");
            let url = format_sqlite_url(&db_path.to_string_lossy());
            let options = SqliteConnectOptions::from_str(&url)
                .expect("sqlite url should be valid")
                .create_if_missing(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .expect("sqlite pool should connect");
            let db = sea_orm::SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);
            tauri_mcp_agent_lib::migration::Migrator::up(&db, None)
                .await
                .expect("migrations should run");
            set_session_repository(SqliteSessionRepository::new(db.clone()));
            TestContext {
                _temp_dir: temp_dir,
                db,
            }
        })
        .await
        .db
        .clone()
}

fn make_session(session_id: &str, workspace_override: Option<String>) -> SessionMetadata {
    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Test Session".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-4.1".to_string(),
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
        created_at: 1,
        updated_at: 1,
        last_viewed_at: None,
        last_message_at: None,
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        execution_mode: ExecutionMode::Normal,
        workspace_override,
        workspace_isolation:
            tauri_mcp_agent_lib::models::workspace_isolation::WorkspaceIsolationMode::Host,
        docker_config: None,
        docker_container_name: None,
        docker_host_workspace_path: None,
    }
}

async fn test_guard() -> MutexGuard<'static, ()> {
    TEST_MUTEX.lock().await
}

fn clamp_result(was_clamped: bool) -> CompactSummaryClampResult {
    CompactSummaryClampResult {
        summary: "clamped".to_string(),
        hard_limit_tokens: 100,
        estimated_tokens: 100,
        original_estimated_tokens: if was_clamped { 900 } else { 100 },
        was_clamped,
    }
}

async fn session_with_workspace(session_id: &str) -> TempDir {
    let db = test_db().await;
    let repo = SqliteSessionRepository::new(db);
    let workspace = tempfile::tempdir().expect("workspace dir should be created");
    repo.upsert_session(&make_session(session_id, None))
        .await
        .expect("session should be persisted");
    WorkspaceService::set_override(session_id, workspace.path().to_string_lossy().to_string())
        .await
        .expect("workspace override should be set");
    workspace
}

fn raw_files(workspace: &TempDir) -> Vec<std::path::PathBuf> {
    let dir = workspace.path().join(".libragent/tool-results/compaction");
    match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("raw-"))
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[tokio::test]
async fn clamped_summary_writes_raw_output_file() {
    let _guard = test_guard().await;
    let session_id = "raw-dump-clamp-session";
    let workspace = session_with_workspace(session_id).await;

    let path = dump_raw_compaction_output_if_needed(
        session_id,
        "msg-1",
        "RAW <verbatim> output",
        &clamp_result(true),
        &Ok(()),
        1,
        "gpt-4.1",
    )
    .await
    .expect("clamp should write a raw dump");

    assert!(path.starts_with(".libragent/tool-results/compaction/raw-"));
    let files = raw_files(&workspace);
    assert_eq!(files.len(), 1);
    let content = std::fs::read_to_string(&files[0]).unwrap();
    assert!(content.contains("- Reason: clamped to hard limit"));
    assert!(content.contains("- model: gpt-4.1"));
    assert!(content.contains("- Raw estimated tokens: 900"));
    assert!(content.ends_with("RAW <verbatim> output\n"));
}

#[tokio::test]
async fn validation_failure_writes_raw_output_file_with_error() {
    let _guard = test_guard().await;
    let session_id = "raw-dump-validation-session";
    let workspace = session_with_workspace(session_id).await;

    let path = dump_raw_compaction_output_if_needed(
        session_id,
        "msg-2",
        "<tool_call>bad",
        &clamp_result(false),
        &Err("Compaction summary contained tool-call markup".to_string()),
        2,
        "gpt-4.1",
    )
    .await
    .expect("validation failure should write a raw dump");

    assert!(path.contains("/raw-"));
    let files = raw_files(&workspace);
    assert_eq!(files.len(), 1);
    let content = std::fs::read_to_string(&files[0]).unwrap();
    assert!(content.contains("validation failed: Compaction summary contained tool-call markup"));
    assert!(content.contains("- Summary retries used: 2"));
    assert!(content.ends_with("<tool_call>bad\n"));
}

#[tokio::test]
async fn clean_unclamped_summary_writes_nothing() {
    let _guard = test_guard().await;
    let session_id = "raw-dump-clean-session";
    let workspace = session_with_workspace(session_id).await;

    let path = dump_raw_compaction_output_if_needed(
        session_id,
        "msg-3",
        "a perfectly fine summary",
        &clamp_result(false),
        &Ok(()),
        0,
        "gpt-4.1",
    )
    .await;

    assert!(path.is_none());
    assert!(raw_files(&workspace).is_empty());
}
