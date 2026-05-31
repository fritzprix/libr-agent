mod common;

use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Set,
    Statement, TransactionTrait,
};
use sea_orm_migration::MigratorTrait;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tauri_mcp_agent_lib::entity::planning_todo;
use tauri_mcp_agent_lib::mcp::builtin::planning::PlanningServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::repositories::{PlanningRepository, SqlitePlanningRepository};
use tauri_mcp_agent_lib::set_planning_repository;
use tauri_mcp_agent_lib::utils::sqlite::format_sqlite_url;

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

async fn connect_sqlite(path: &str) -> DatabaseConnection {
    let mut options = sea_orm::ConnectOptions::new(path.to_string());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("sqlite connection should open");
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "PRAGMA busy_timeout = 50".to_string(),
    ))
    .await
    .expect("busy timeout pragma should apply");
    db
}

async fn insert_locked_todo(txn: &sea_orm::DatabaseTransaction, session_id: &str, label: &str) {
    let now = chrono::Utc::now().timestamp_millis();
    planning_todo::ActiveModel {
        session_id: Set(session_id.to_string()),
        content: Set(label.to_string()),
        description: Set(Some(label.to_string())),
        priority: Set("medium".to_string()),
        status: Set("pending".to_string()),
        is_checked: Set(false),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(txn)
    .await
    .expect("lock row insert should succeed");
}

#[tokio::test]
async fn planning_writes_retry_busy_locks_and_hide_raw_db_errors() {
    common::register_sqlite_vec();
    let temp_dir = tempfile::tempdir().expect("temp dir should exist");
    let db_path = temp_dir.path().join("planning-lock-test.sqlite");
    let database_url = format!("{}?mode=rwc", format_sqlite_url(&db_path.to_string_lossy()));

    let db1 = connect_sqlite(&database_url).await;
    Migrator::up(&db1, None)
        .await
        .expect("migrations should apply");
    let db2 = connect_sqlite(&database_url).await;

    let direct_repo = Arc::new(SqlitePlanningRepository::new(db2.clone()));
    let retry_txn = db1.begin().await.expect("lock transaction should start");
    insert_locked_todo(&retry_txn, "retry-session", "lock-retry").await;

    let retry_task = {
        let repo = Arc::clone(&direct_repo);
        tokio::spawn(async move {
            repo.add_todo(
                "retry-session",
                "Retried todo",
                Some("Retried todo".to_string()),
                "high",
            )
            .await
        })
    };

    tokio::time::sleep(Duration::from_millis(100)).await;
    retry_txn
        .rollback()
        .await
        .expect("retry lock should release cleanly");

    let todo_id = retry_task
        .await
        .expect("retry task should join")
        .expect("repo should retry and succeed");
    assert!(todo_id > 0, "retry path should create a todo");

    set_planning_repository(SqlitePlanningRepository::new(db2.clone()));
    let server = PlanningServer::new("busy-session".to_string(), Arc::new(db2))
        .await
        .expect("planning server should initialize");

    let stuck_txn = db1.begin().await.expect("stuck transaction should start");
    insert_locked_todo(&stuck_txn, "busy-session", "lock-stuck").await;

    let result = server
        .call_tool(
            "addTodo",
            json!({
                "description": "Todo that should surface a normalized busy error",
                "priority": "medium"
            }),
            None,
        )
        .await
        .expect("tool should return an MCP result");

    let text = extract_text(&result);
    assert_eq!(result.is_error, Some(true));
    assert!(
        text.contains("Planning storage was temporarily busy"),
        "expected normalized busy message, got: {text}"
    );
    assert!(
        !text.contains("SeaORM")
            && !text.contains("database is locked")
            && !text.contains("code: 5"),
        "raw database details should not leak: {text}"
    );

    stuck_txn
        .rollback()
        .await
        .expect("stuck lock should release cleanly");
}
