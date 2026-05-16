mod common;

use migration::MigratorTrait;
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sea_orm::{DatabaseConnection, SqlxSqliteConnector};
use serde_json::json;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri_mcp_agent_lib::mcp::builtin::agent::handlers::{
    list_agent_configs_for_test, list_delegated_sessions_for_test,
};
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::repositories::{
    AssistantRepository, SessionMetadata, SessionRepository, SessionStatus,
    SqliteAssistantRepository, SqliteMCPServerRepository, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::utils::sqlite::format_sqlite_url;
use tauri_mcp_agent_lib::{set_mcp_server_repository, set_session_repository};
use tempfile::TempDir;
use tokio::sync::{Mutex, OnceCell};

struct TestContext {
    _temp_dir: TempDir,
    db: DatabaseConnection,
}

static TEST_CONTEXT: OnceCell<TestContext> = OnceCell::const_new();
static TEST_GUARD: Mutex<()> = Mutex::const_new(());
static TEST_ID: AtomicU64 = AtomicU64::new(0);

async fn test_db() -> DatabaseConnection {
    TEST_CONTEXT
        .get_or_init(|| async {
            common::register_sqlite_vec();
            let temp_dir = tempfile::tempdir().expect("temp dir should create");
            let db_path = temp_dir.path().join("agent-list-pagination-tests.db");
            let url = format_sqlite_url(&db_path.to_string_lossy());
            let options = SqliteConnectOptions::from_str(&url)
                .expect("sqlite url should be valid")
                .create_if_missing(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .expect("sqlite pool should connect");
            let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);
            Migrator::up(&db, None)
                .await
                .expect("migrations should run");
            set_session_repository(SqliteSessionRepository::new(db.clone()));
            set_mcp_server_repository(SqliteMCPServerRepository::new(db.clone()));
            TestContext {
                _temp_dir: temp_dir,
                db,
            }
        })
        .await
        .db
        .clone()
}

fn extract_text(result: &MCPResult) -> String {
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

fn next_test_id() -> u64 {
    TEST_ID.fetch_add(1, Ordering::Relaxed)
}

fn build_session(
    id: &str,
    name: &str,
    parent_session_id: Option<&str>,
    status: SessionStatus,
    timestamp: i64,
) -> SessionMetadata {
    SessionMetadata {
        id: id.to_string(),
        name: Some(name.to_string()),
        status,
        model: "gpt-4.1".to_string(),
        provider: "openai".to_string(),
        agent_config: None,
        parent_session_id: parent_session_id.map(ToOwned::to_owned),
        lineage_id: Some(format!("lineage-{id}")),
        depth: Some(if parent_session_id.is_some() { 1 } else { 0 }),
        max_depth: None,
        max_fanout: None,
        org_id: None,
        org_name: None,
        org_root_session_id: None,
        created_at: timestamp,
        updated_at: timestamp,
        last_viewed_at: None,
        last_message_at: Some(timestamp),
        last_attention_at: None,
        last_attention_reason: None,
        is_bookmarked: false,
        yolo_mode: false,
        unsafe_mode: false,
        workspace_override: None,
    }
}

#[tokio::test]
async fn list_configs_keeps_table_contiguous_and_adds_pagination_note_after_rows() {
    let _guard = TEST_GUARD.lock().await;
    let db = test_db().await;
    let repo = SqliteAssistantRepository::new(db.clone());
    let test_id = next_test_id();
    let query = format!("cfg-pagination-{test_id}");

    for index in 0..3 {
        repo.create_assistant(
            format!("agent-{test_id}-{index}"),
            format!("{query}-agent-{index}"),
            json!({
                "description": format!("{query} description {index}"),
                "allowedBuiltInServiceAliases": ["workspace"],
                "mcpServerIds": []
            })
            .to_string(),
        )
        .await
        .expect("assistant should be created");
    }

    let result = list_agent_configs_for_test(
        &db,
        &json!({
            "query": query,
            "limit": 2,
            "offset": 1
        }),
    )
    .await
    .expect("list should succeed");

    let text = extract_text(&result);
    assert!(text.contains("|---|---|---|---|---|\n|"));
    assert!(text.contains("Showing 2 to 3 of 3 items"));
}

#[tokio::test]
async fn list_configs_empty_page_reports_no_results_without_invalid_range() {
    let _guard = TEST_GUARD.lock().await;
    let db = test_db().await;
    let repo = SqliteAssistantRepository::new(db.clone());
    let test_id = next_test_id();
    let query = format!("cfg-empty-{test_id}");

    repo.create_assistant(
        format!("agent-empty-{test_id}"),
        format!("{query}-agent"),
        json!({
            "description": format!("{query} description"),
            "allowedBuiltInServiceAliases": ["workspace"],
            "mcpServerIds": []
        })
        .to_string(),
    )
    .await
    .expect("assistant should be created");

    let result = list_agent_configs_for_test(
        &db,
        &json!({
            "query": query,
            "limit": 2,
            "offset": 5
        }),
    )
    .await
    .expect("list should succeed");

    let text = extract_text(&result);
    assert!(text.contains("No results for this page (offset 5, limit 2). Try a smaller offset."));
    assert!(!text.contains("Showing 6 to 5"));
}

#[tokio::test]
async fn list_sessions_paginates_child_ids_before_rendering_current_page() {
    let _guard = TEST_GUARD.lock().await;
    let db = test_db().await;
    let repo = SqliteSessionRepository::new(db);
    let test_id = next_test_id();
    let parent_id = format!("parent-session-{test_id}");

    repo.upsert_session(&build_session(
        &parent_id,
        "Parent Session",
        None,
        SessionStatus::Idle,
        1_700_100_000_000,
    ))
    .await
    .expect("parent session should be created");

    for index in 0..25 {
        let child_id = format!("child-session-{test_id}-{index}");
        repo.upsert_session(&build_session(
            &child_id,
            &format!("Child Session {index}"),
            Some(&parent_id),
            if index % 2 == 0 {
                SessionStatus::Idle
            } else {
                SessionStatus::Busy
            },
            1_700_100_000_100 + index as i64,
        ))
        .await
        .expect("child session should be created");
    }

    let result = list_delegated_sessions_for_test(
        &parent_id,
        &json!({
            "limit": 2,
            "offset": 20
        }),
    )
    .await
    .expect("list should succeed");

    let text = extract_text(&result);
    assert!(text.contains("Found 25 sub-agent sessions."));
    assert!(text.contains("|---|---|---|\n|"));
    assert!(text.contains(
        "Showing 21 to 22 of 25 items. Call this tool again with offset: 22 to see more"
    ));

    let structured = result
        .structured_content
        .expect("structured content expected");
    assert_eq!(structured["total"], 25);
    assert_eq!(
        structured["sessions"]
            .as_array()
            .expect("sessions array expected")
            .len(),
        2
    );
}

#[tokio::test]
async fn list_sessions_empty_page_reports_no_results_without_invalid_range() {
    let _guard = TEST_GUARD.lock().await;
    let db = test_db().await;
    let repo = SqliteSessionRepository::new(db);
    let test_id = next_test_id();
    let parent_id = format!("parent-empty-{test_id}");

    repo.upsert_session(&build_session(
        &parent_id,
        "Parent Empty Session",
        None,
        SessionStatus::Idle,
        1_700_200_000_000,
    ))
    .await
    .expect("parent session should be created");

    for index in 0..3 {
        let child_id = format!("child-empty-{test_id}-{index}");
        repo.upsert_session(&build_session(
            &child_id,
            &format!("Child Empty {index}"),
            Some(&parent_id),
            SessionStatus::Idle,
            1_700_200_000_100 + index as i64,
        ))
        .await
        .expect("child session should be created");
    }

    let result = list_delegated_sessions_for_test(
        &parent_id,
        &json!({
            "limit": 2,
            "offset": 5
        }),
    )
    .await
    .expect("list should succeed");

    let text = extract_text(&result);
    assert!(text.contains("No results for this page (offset 5, limit 2). Try a smaller offset."));
    assert!(!text.contains("Showing 6 to 5"));
}
