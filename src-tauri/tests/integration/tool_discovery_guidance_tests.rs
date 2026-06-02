use crate::common;

use migration::MigratorTrait;
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sea_orm::{DatabaseConnection, SqlxSqliteConnector};
use serde_json::json;
use std::str::FromStr;
use tauri_mcp_agent_lib::mcp::builtin::tool::ToolServer;
use tauri_mcp_agent_lib::mcp::builtin::BuiltinMCPServer;
use tauri_mcp_agent_lib::mcp::types::MCPContent;
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::repositories::{
    MCPServerRepository, SessionMetadata, SessionRepository, SessionStatus,
    SqliteMCPServerRepository, SqliteSessionRepository,
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
            let db_path = temp_dir.path().join("tool-discovery-tests.db");
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

#[tokio::test]
async fn tool_list_uses_canonical_agent_update_guidance() {
    let db = test_db().await;

    let server_repo = SqliteMCPServerRepository::new(db.clone());
    let created = server_repo
        .create(
            "github",
            json!({
            "name": "github",
            "transport": { "type": "http", "url": "https://example.com/mcp" }
            }),
        )
        .await
        .expect("mcp server should insert");
    server_repo
        .update_cached_tools(
            &created.id,
            1,
            json!([{ "name": "search_issues", "description": "Search repo issues" }]).to_string(),
        )
        .await
        .expect("cached tools should update");

    let session_repo = SqliteSessionRepository::new(db.clone());
    session_repo
        .upsert_session(&SessionMetadata {
            id: "session-tool-list".to_string(),
            name: Some("Tool List Session".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4.1".to_string(),
            provider: "openai".to_string(),
            agent_config: Some(
                json!({
                    "assistantId": "agent-alpha",
                    "name": "Agent Alpha",
                    "systemPrompt": "You are helpful",
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

    let result = ToolServer::new()
        .call_tool(
            "list",
            json!({ "scope": "external" }),
            Some("session-tool-list".to_string()),
        )
        .await
        .expect("list_tools should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("Server IDs found:"),
        "inventory mode should still summarize discovered external server ids: {text}"
    );
    assert!(
        text.contains("agent__update"),
        "tool discovery guidance should point to canonical agent__update: {text}"
    );
    assert!(
        !text.contains("updateAssistant("),
        "tool discovery guidance should not mention legacy updateAssistant alias: {text}"
    );
    assert!(
        !text.contains("[Requires agent__update]"),
        "inventory mode should not annotate session readiness by default: {text}"
    );
    assert!(
        text.contains("availability: inventory"),
        "inventory mode header should be explicit: {text}"
    );
}

#[tokio::test]
async fn tool_list_marks_unavailable_external_servers_as_unsupported_in_current_session() {
    let db = test_db().await;

    let server_repo = SqliteMCPServerRepository::new(db.clone());
    let created = server_repo
        .create(
            "research",
            json!({
            "name": "research",
            "transport": { "type": "http", "url": "https://example.com/mcp" }
            }),
        )
        .await
        .expect("mcp server should insert");
    server_repo
        .update_cached_tools(
            &created.id,
            1,
            json!([{ "name": "web_search", "description": "Search the web" }]).to_string(),
        )
        .await
        .expect("cached tools should update");

    let session_repo = SqliteSessionRepository::new(db.clone());
    session_repo
        .upsert_session(&SessionMetadata {
            id: "session-tool-status".to_string(),
            name: Some("Tool Status Session".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4.1".to_string(),
            provider: "openai".to_string(),
            agent_config: Some(
                json!({
                    "assistantId": "agent-beta",
                    "name": "Agent Beta",
                    "systemPrompt": "You are helpful",
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

    let result = ToolServer::new()
        .call_tool(
            "list",
            json!({
                "scope": "external",
                "query": "web_search",
                "availability": "session"
            }),
            Some("session-tool-status".to_string()),
        )
        .await
        .expect("list_tools should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("[Unsupported in current session]"),
        "session mode should show external availability status: {text}"
    );
    assert!(
        !text.contains("agent__update"),
        "session mode should not suggest self-update or follow-up update actions: {text}"
    );
    assert!(
        text.contains("availability: session"),
        "session mode header should be explicit: {text}"
    );
}

#[tokio::test]
async fn tool_list_uses_builtin_service_alias_for_session_status() {
    let db = test_db().await;

    let session_repo = SqliteSessionRepository::new(db.clone());
    session_repo
        .upsert_session(&SessionMetadata {
            id: "session-builtin-status".to_string(),
            name: Some("Builtin Status Session".to_string()),
            status: SessionStatus::Idle,
            model: "gpt-4.1".to_string(),
            provider: "openai".to_string(),
            agent_config: Some(
                json!({
                    "assistantId": "agent-gamma",
                    "name": "Agent Gamma",
                    "systemPrompt": "You are helpful",
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

    let result = ToolServer::new()
        .call_tool(
            "list",
            json!({
                "scope": "internal",
                "query": "createGoal",
                "availability": "session"
            }),
            Some("session-builtin-status".to_string()),
        )
        .await
        .expect("list_tools should succeed");

    let text = extract_text(&result);
    assert!(
        text.contains("| Builtin | createGoal | [Ready] |"),
        "planning tool should inherit planning service readiness instead of using tool name as alias: {text}"
    );
    assert!(
        !text.contains("This session cannot call 'createGoal' tools right now"),
        "builtin status should not be derived from the bare tool name: {text}"
    );
}
