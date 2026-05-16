mod common;

use migration::MigratorTrait;
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sea_orm::{DatabaseConnection, SqlxSqliteConnector};
use serde_json::json;
use std::str::FromStr;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tauri_mcp_agent_lib::mcp::builtin::scratchpad::ScratchpadServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::repositories::{PlanningRepository, SqlitePlanningRepository};
use tauri_mcp_agent_lib::set_planning_repository;
use tauri_mcp_agent_lib::utils::sqlite::format_sqlite_url;
use tempfile::TempDir;
use tokio::sync::{Mutex, OnceCell};

struct TestContext {
    _temp_dir: TempDir,
    db: Arc<DatabaseConnection>,
}

static TEST_CONTEXT: OnceCell<TestContext> = OnceCell::const_new();
static TEST_GUARD: Mutex<()> = Mutex::const_new(());
static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

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

async fn test_db() -> Arc<DatabaseConnection> {
    TEST_CONTEXT
        .get_or_init(|| async {
            common::register_sqlite_vec();
            let temp_dir = tempfile::tempdir().expect("temp dir should create");
            let db_path = temp_dir.path().join("scratchpad-list-format-tests.db");
            let url = format_sqlite_url(&db_path.to_string_lossy());
            let options = SqliteConnectOptions::from_str(&url)
                .expect("sqlite url should be valid")
                .create_if_missing(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .expect("sqlite pool should connect");
            let db = Arc::new(SqlxSqliteConnector::from_sqlx_sqlite_pool(pool));
            Migrator::up(&*db, None)
                .await
                .expect("migrations should run");
            set_planning_repository(SqlitePlanningRepository::new((*db).clone()));
            TestContext {
                _temp_dir: temp_dir,
                db,
            }
        })
        .await
        .db
        .clone()
}

fn next_id() -> u64 {
    TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[tokio::test]
async fn scratchpad_list_renders_markdown_table_and_escapes_cells() {
    let _guard = TEST_GUARD.lock().await;
    let db = test_db().await;
    let repo = SqlitePlanningRepository::new((*db).clone());
    let test_id = next_id();
    let session_id = format!("scratchpad-list-format-{test_id}");

    repo.add_scratchpad(
        &session_id,
        Some("Title | Main\nLine".to_string()),
        "Body line 1 |\nBody line 2",
        None,
        Some(json!(["tag|one", "tag\ntwo"]).to_string()),
    )
    .await
    .expect("first scratchpad note should be created");
    repo.add_scratchpad(
        &session_id,
        Some("Second note".to_string()),
        "Second body",
        None,
        None,
    )
    .await
    .expect("second scratchpad note should be created");

    let server = ScratchpadServer::new(session_id.clone(), db)
        .await
        .expect("scratchpad server should initialize");
    let result = server
        .call_tool(
            "list",
            json!({
                "page": 2,
                "pageSize": 1
            }),
            Some(session_id),
        )
        .await
        .expect("list should succeed");

    let text = extract_text(&result);
    assert!(text.contains("| ID | Title | Preview | Tags |"));
    assert!(text.contains("|---|---|---|---|"));
    assert!(text.contains("Title \\| Main Line"));
    assert!(text.contains("Body line 1 \\| Body line 2"));
    assert!(text.contains("tag\\|one, tag two"));
    assert!(text.contains("Showing 2 to 2 of 2 items"));
}

#[tokio::test]
async fn scratchpad_list_rejects_overflowing_pagination_inputs() {
    let _guard = TEST_GUARD.lock().await;
    let db = test_db().await;
    let session_id = format!("scratchpad-pagination-overflow-{}", next_id());
    let server = ScratchpadServer::new(session_id.clone(), db)
        .await
        .expect("scratchpad server should initialize");

    let result = server
        .call_tool(
            "list",
            json!({
                "page": i64::MAX,
                "pageSize": 2
            }),
            Some(session_id),
        )
        .await
        .expect("list should return a guided error");

    assert_eq!(result.is_error, Some(true));
    assert!(extract_text(&result).contains("page and pageSize combination is too large"));
}
