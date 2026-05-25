mod common;

use migration::MigratorTrait;
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, SqlxSqliteConnector, Statement,
};
use serde_json::{json, Value};
use std::str::FromStr;
use tauri_mcp_agent_lib::mcp::builtin::tool::ToolServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::repositories::{MCPServerRepository, SqliteMCPServerRepository};
use tauri_mcp_agent_lib::set_mcp_server_repository;
use tauri_mcp_agent_lib::utils::sqlite::format_sqlite_url;
use tempfile::TempDir;
use tokio::sync::OnceCell;

struct TestContext {
    _temp_dir: TempDir,
    db: DatabaseConnection,
    repo: SqliteMCPServerRepository,
}

static TEST_CONTEXT: OnceCell<TestContext> = OnceCell::const_new();

async fn repo() -> SqliteMCPServerRepository {
    TEST_CONTEXT
        .get_or_init(|| async {
            common::register_sqlite_vec();
            let temp_dir = tempfile::tempdir().expect("temp dir should create");
            let db_path = temp_dir.path().join("tool-contract-tests.db");
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
            let repo = SqliteMCPServerRepository::new(db.clone());
            set_mcp_server_repository(repo.clone());
            TestContext {
                _temp_dir: temp_dir,
                db,
                repo,
            }
        })
        .await
        .repo
        .clone()
}

async fn db() -> DatabaseConnection {
    TEST_CONTEXT
        .get()
        .expect("test context should be initialized")
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

#[tokio::test]
async fn register_rejects_duplicate_server_names_without_mutating_existing_config() {
    let repo = repo().await;
    let server = ToolServer::new();

    let existing = repo
        .create(
            "github-duplicate-check",
            json!({
                "name": "github-duplicate-check",
                "transport": {
                    "type": "http",
                    "url": "https://example.com/original"
                }
            }),
        )
        .await
        .expect("existing server should insert");

    let result = server
        .call_tool(
            "register",
            json!({
                "name": "github-duplicate-check",
                "description": "Replacement config that should be rejected",
                "transport": {
                    "type": "http",
                    "url": "https://127.0.0.1:1/should-not-run"
                }
            }),
            None,
        )
        .await
        .expect("register should return an MCP result");

    let text = extract_text(&result);
    assert_eq!(result.is_error, Some(true));
    assert!(text.contains("already exists"));
    assert!(text.contains("update(name=\"github-duplicate-check\""));

    let reloaded = repo
        .get(&existing.id)
        .await
        .expect("existing server should reload")
        .expect("existing server should still exist");
    let config: Value =
        serde_json::from_str(&reloaded.config).expect("stored config should remain valid JSON");
    assert_eq!(
        config["transport"]["url"].as_str(),
        Some("https://example.com/original")
    );
}

#[tokio::test]
async fn register_aborts_when_existing_server_config_cannot_be_loaded() {
    let repo = repo().await;
    let db = db().await;
    let server = ToolServer::new();

    let existing = repo
        .create(
            "github-broken-config-check",
            json!({
                "name": "github-broken-config-check",
                "transport": {
                    "type": "http",
                    "url": "https://example.com/original"
                }
            }),
        )
        .await
        .expect("existing server should insert");

    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE mcp_servers SET config = ? WHERE id = ?",
        ["{".into(), existing.id.clone().into()],
    ))
    .await
    .expect("test setup should corrupt existing config");

    let result = server
        .call_tool(
            "register",
            json!({
                "name": "github-broken-config-check",
                "description": "Replacement config that should be rejected on lookup error",
                "transport": {
                    "type": "http",
                    "url": "https://127.0.0.1:1/should-not-run"
                }
            }),
            None,
        )
        .await
        .expect("register should return an MCP result");

    let text = extract_text(&result);
    assert_eq!(result.is_error, Some(false));
    assert!(
        text.contains("Failed to check whether server 'github-broken-config-check' already exists")
    );
    assert!(text.contains("registration was aborted"));

    let reloaded = repo
        .get(&existing.id)
        .await
        .expect("existing server should reload")
        .expect("existing server should still exist");
    assert_eq!(reloaded.config, "{");
}

#[tokio::test]
async fn list_returns_structured_results_for_ui_consumers() {
    let repo = repo().await;
    let server = ToolServer::new();

    let created = repo
        .create(
            "github-structured-list",
            json!({
                "name": "github-structured-list",
                "transport": {
                    "type": "http",
                    "url": "https://example.com/mcp"
                }
            }),
        )
        .await
        .expect("server should insert");

    repo.update_cached_tools(
        &created.id,
        1,
        json!([
            {
                "name": "search_issues",
                "description": "Search repository issues"
            }
        ])
        .to_string(),
    )
    .await
    .expect("cached tools should update");

    let result = server
        .call_tool(
            "list",
            json!({
                "query": "search_issues",
                "scope": "external",
                "availability": "inventory"
            }),
            None,
        )
        .await
        .expect("list should return an MCP result");

    let structured = result
        .structured_content
        .clone()
        .expect("structured content should be present");
    let external_servers = structured["externalServers"]
        .as_array()
        .expect("externalServers should be an array");

    let text = extract_text(&result);
    assert!(text.contains("| External: github-structured-list | search_issues |"));
    assert_eq!(structured["totalResults"].as_u64(), Some(1));
    assert!(external_servers.iter().any(|entry| {
        entry["id"].as_str() == Some(created.id.as_str())
            && entry["name"].as_str() == Some("github-structured-list")
    }));
}
