#![cfg(not(windows))]

mod common;

use migration::MigratorTrait;
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sea_orm::{DatabaseConnection, SqlxSqliteConnector};
use serde_json::json;
use std::str::FromStr;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::service_proxy_manager::MCPServiceProxyManager;
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResponseResult};
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::repositories::{
    MCPServerRepository, SessionMetadata, SessionRepository, SessionStatus,
    SqliteMCPServerRepository, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::session::SessionManager;
use tauri_mcp_agent_lib::utils::sqlite::format_sqlite_url;
use tauri_mcp_agent_lib::{set_mcp_server_repository, set_session_repository};
use tempfile::TempDir;
use tokio::sync::OnceCell;

struct TestContext {
    db: Arc<DatabaseConnection>,
    _temp_dir: TempDir,
}

static TEST_CONTEXT: OnceCell<TestContext> = OnceCell::const_new();

async fn test_context() -> &'static TestContext {
    TEST_CONTEXT
        .get_or_init(|| async {
            common::register_sqlite_vec();
            let temp_dir = tempfile::tempdir().expect("temp dir should create");
            let db_path = temp_dir.path().join("external-tool-recovery-tests.db");
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
            set_mcp_server_repository(SqliteMCPServerRepository::new((*db).clone()));
            set_session_repository(SqliteSessionRepository::new((*db).clone()));
            TestContext {
                db,
                _temp_dir: temp_dir,
            }
        })
        .await
}

fn extract_text(response: &tauri_mcp_agent_lib::mcp::types::MCPResponse) -> String {
    let result = response.result.as_ref().expect("tool result");
    let MCPResponseResult::ToolCall(result) = result else {
        panic!("expected tool-call result");
    };
    result
        .content
        .as_ref()
        .expect("text content")
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn detached_external_server_returns_delegate_or_attach_guidance() {
    let context = test_context().await;
    let db = context.db.clone();

    let session_repo = SqliteSessionRepository::new((*db).clone());
    let server_repo = SqliteMCPServerRepository::new((*db).clone());

    let session_id = "external-recovery-session";
    session_repo
        .upsert_session(&SessionMetadata {
            id: session_id.to_string(),
            name: Some("External Recovery Session".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4.1".to_string(),
            provider: "openai".to_string(),
            agent_config: Some(
                json!({
                    "assistantId": "agent-recovery",
                    "name": "Recovery Agent",
                    "systemPrompt": "You recover intelligently.",
                    "allowedBuiltInServiceAliases": ["planning", "workspace", "agent", "tool", "attachments", "ui", "skills", "playbook", "scratchpad"],
                    "mcpServerIds": []
                })
                .to_string(),
            ),
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

    let created = server_repo
        .create(
            "grok",
            json!({
                "name": "grok",
                "transport": { "type": "http", "url": "https://example.com/mcp" }
            }),
        )
        .await
        .expect("server should insert");

    let session_root = TempDir::new().expect("temp dir");
    let session_manager = Arc::new(
        SessionManager::new_with_base_dir(session_root.path().join("session-root"))
            .expect("session manager"),
    );
    let manager = MCPServiceProxyManager::new(db, session_manager);

    let response = manager
        .call_tool(
            session_id,
            "grok__search_web",
            json!({ "query": "S&P 500 gold oil price May 2026" }),
        )
        .await
        .expect("guided tool error should still return MCPResponse");

    let text = extract_text(&response);
    let result = response.result.as_ref().expect("tool result");
    let MCPResponseResult::ToolCall(result) = result else {
        panic!("expected tool-call result");
    };

    assert_eq!(result.is_error, Some(true));
    assert!(
        text.contains("Category: PermissionDenied"),
        "should classify detached global server as permission/session-attachment issue: {text}"
    );
    assert!(
        text.contains(&format!("Server ID: {}", created.id)),
        "guidance should surface the concrete server id needed for attachment: {text}"
    );
    assert!(
        text.contains("tool__list({\"availability\":\"session\"})"),
        "guidance should teach session-visible inventory inspection: {text}"
    );
    assert!(
        text.contains("tool__list({\"availability\":\"inventory\"})"),
        "guidance should teach global inventory inspection: {text}"
    );
    assert!(
        text.contains("agent__update"),
        "guidance should explain how to attach the missing server: {text}"
    );
    assert!(
        text.contains("agent__list(type=\"configs\")"),
        "guidance should point toward alternative agents: {text}"
    );
    assert!(
        text.contains("agent__startSession"),
        "guidance should explicitly mention delegation as a recovery path: {text}"
    );
}

#[tokio::test]
async fn external_call_reconfigures_existing_builtin_only_proxy() {
    let context = test_context().await;
    let db = context.db.clone();

    let session_repo = SqliteSessionRepository::new((*db).clone());
    let server_repo = SqliteMCPServerRepository::new((*db).clone());

    let created = server_repo
        .create(
            "grok-configured",
            json!({
                "name": "grok-configured",
                "transport": { "type": "http", "url": "https://example.com/mcp" }
            }),
        )
        .await
        .expect("server should insert");

    let session_id = "external-recovery-configured-session";
    session_repo
        .upsert_session(&SessionMetadata {
            id: session_id.to_string(),
            name: Some("External Recovery Configured Session".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4.1".to_string(),
            provider: "openai".to_string(),
            agent_config: Some(
                json!({
                    "assistantId": "agent-recovery-configured",
                    "name": "Recovery Agent Configured",
                    "systemPrompt": "You recover intelligently.",
                    "allowedBuiltInServiceAliases": ["planning", "workspace", "agent", "tool", "attachments", "ui", "skills", "playbook", "scratchpad"],
                    "mcpServerIds": [created.id.clone()]
                })
                .to_string(),
            ),
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

    let session_root = TempDir::new().expect("temp dir");
    let session_manager = Arc::new(
        SessionManager::new_with_base_dir(session_root.path().join("session-root"))
            .expect("session manager"),
    );
    let manager = MCPServiceProxyManager::new(db, session_manager);

    manager
        .call_tool(
            session_id,
            "tool__list",
            json!({ "availability": "session" }),
        )
        .await
        .expect("builtin tool should succeed and create a lazy builtin-only proxy");

    let initial_proxy = manager
        .get_proxy(session_id)
        .await
        .expect("builtin call should create proxy");
    assert!(
        initial_proxy
            .configured_external_server_names()
            .await
            .is_empty(),
        "lazy builtin proxy should not yet have configured external servers"
    );

    let response = manager
        .call_tool(
            session_id,
            "grok-configured__search_web",
            json!({ "query": "S&P 500 gold oil price May 2026" }),
        )
        .await
        .expect("configured external call should return an MCP response");

    let text = extract_text(&response);
    let reconfigured_proxy = manager
        .get_proxy(session_id)
        .await
        .expect("external call should keep a proxy");
    let configured_servers = reconfigured_proxy.configured_external_server_names().await;

    assert!(
        configured_servers
            .iter()
            .any(|server_name| server_name == "grok-configured"),
        "external call should replace builtin-only proxy with config-aware proxy containing attached external servers"
    );
    assert!(
        !text.contains("Category: PermissionDenied"),
        "configured attached server must not be misreported as detached from the session: {text}"
    );
}
