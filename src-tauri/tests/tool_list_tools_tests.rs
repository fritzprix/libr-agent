mod common;

use migration::MigratorTrait;
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sea_orm::{DatabaseConnection, SqlxSqliteConnector};
use serde_json::json;
use std::str::FromStr;
use tauri_mcp_agent_lib::mcp::builtin::tool::ToolServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::repositories::{MCPServerRepository, SessionMetadata, SessionStatus};
use tauri_mcp_agent_lib::repositories::{
    SessionRepository, SqliteMCPServerRepository, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::utils::sqlite::format_sqlite_url;
use tauri_mcp_agent_lib::{set_mcp_server_repository, set_session_repository};
use tempfile::TempDir;
use tokio::sync::OnceCell;

struct TestContext {
    _temp_dir: TempDir,
    db: DatabaseConnection,
}

static TEST_CONTEXT: OnceCell<TestContext> = OnceCell::const_new();

async fn test_db() -> DatabaseConnection {
    TEST_CONTEXT
        .get_or_init(|| async {
            common::register_sqlite_vec();
            let temp_dir = tempfile::tempdir().expect("temp dir should create");
            let db_path = temp_dir.path().join("tool-list-tools-tests.db");
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
            set_mcp_server_repository(SqliteMCPServerRepository::new(db.clone()));
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

fn extract_text(result: &tauri_mcp_agent_lib::mcp::types::MCPResult) -> String {
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

async fn upsert_inventory_session(db: &DatabaseConnection, session_id: &str) {
    let session_repo = SqliteSessionRepository::new(db.clone());
    session_repo
        .upsert_session(&SessionMetadata {
            id: session_id.to_string(),
            name: Some("Tool List Test Session".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4.1".to_string(),
            provider: "openai".to_string(),
            agent_config: None,
            parent_session_id: None,
            lineage_id: None,
            depth: Some(0),
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
        })
        .await
        .expect("session should insert");
}

#[tokio::test]
async fn list_tools_limits_server_ids_to_visible_page() {
    let db = test_db().await;
    upsert_inventory_session(&db, "tool-page-bounded-session").await;

    let repo = SqliteMCPServerRepository::new(db.clone());
    let first = repo
        .create(
            "page-bound-alpha",
            json!({
                "name": "page-bound-alpha",
                "transport": { "type": "http", "url": "https://example.com/mcp/alpha" }
            }),
        )
        .await
        .expect("first server should insert");
    repo.update_cached_tools(
        &first.id,
        1,
        json!([{ "name": "alpha_tool", "description": "Alpha tool" }]).to_string(),
    )
    .await
    .expect("first cached tools should update");

    let second = repo
        .create(
            "page-bound-beta",
            json!({
                "name": "page-bound-beta",
                "transport": { "type": "http", "url": "https://example.com/mcp/beta" }
            }),
        )
        .await
        .expect("second server should insert");
    repo.update_cached_tools(
        &second.id,
        1,
        json!([{ "name": "beta_tool", "description": "Beta tool" }]).to_string(),
    )
    .await
    .expect("second cached tools should update");

    let result = ToolServer::new()
        .call_tool(
            "list",
            json!({
                "scope": "external",
                "query": "page-bound-",
                "limit": 1
            }),
            Some("tool-page-bounded-session".to_string()),
        )
        .await
        .expect("list should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("Found 2 tools matching 'page-bound-'"),
        "tool count should reflect matching tools, not hidden pages: {text}"
    );
    assert!(
        text.contains("Server IDs found:\n(this page only)"),
        "server-id guidance should be page-bounded: {text}"
    );
    assert_eq!(
        text.matches("→ \"").count(),
        1,
        "only the visible page's server ids should be listed: {text}"
    );
    assert!(
        text.contains("offset: 1"),
        "paginated output should point to the next page: {text}"
    );
}

#[tokio::test]
async fn list_tools_distinguishes_placeholder_server_rows_from_tool_count() {
    let db = test_db().await;
    upsert_inventory_session(&db, "tool-placeholder-session").await;

    let repo = SqliteMCPServerRepository::new(db.clone());
    repo.create(
        "empty-placeholder",
        json!({
            "name": "empty-placeholder",
            "transport": { "type": "http", "url": "https://example.com/mcp/empty" }
        }),
    )
    .await
    .expect("placeholder server should insert");

    let result = ToolServer::new()
        .call_tool(
            "list",
            json!({
                "scope": "external",
                "query": "empty-placeholder"
            }),
            Some("tool-placeholder-session".to_string()),
        )
        .await
        .expect("list should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("Found 0 tools and 1 matching servers without cached tools matching 'empty-placeholder'"),
        "placeholder rows should not be counted as tools: {text}"
    );
    assert!(
        text.contains("(No tools cached. Run with forceVerify=true to discover tools)"),
        "the empty-cache hint should explain what forceVerify does: {text}"
    );
}

#[tokio::test]
async fn list_tools_rejects_offsets_past_total_results() {
    let db = test_db().await;
    upsert_inventory_session(&db, "tool-offset-session").await;

    let repo = SqliteMCPServerRepository::new(db.clone());
    let created = repo
        .create(
            "offset-check",
            json!({
                "name": "offset-check",
                "transport": { "type": "http", "url": "https://example.com/mcp/offset" }
            }),
        )
        .await
        .expect("offset server should insert");
    repo.update_cached_tools(
        &created.id,
        1,
        json!([{ "name": "offset_tool", "description": "Offset tool" }]).to_string(),
    )
    .await
    .expect("offset cached tools should update");

    let result = ToolServer::new()
        .call_tool(
            "list",
            json!({
                "scope": "external",
                "query": "offset-check",
                "offset": 5
            }),
            Some("tool-offset-session".to_string()),
        )
        .await
        .expect("list should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("Offset 5 exceeds total results (1). Try calling again with offset: 0"),
        "out-of-range offsets should return recovery guidance instead of an empty table: {text}"
    );
    assert!(
        text.contains("Reset offset to 0"),
        "offset recovery guidance should be explicit: {text}"
    );
}
