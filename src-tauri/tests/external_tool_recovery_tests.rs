#![cfg(not(windows))]

mod common;

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::service_proxy_manager::MCPServiceProxyManager;
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResponseResult};
use tauri_mcp_agent_lib::repositories::{
    MCPServerRepository, SessionMetadata, SessionRepository, SessionStatus,
    SqliteMCPServerRepository, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::session::SessionManager;
use tauri_mcp_agent_lib::{set_mcp_server_repository, set_session_repository};
use tempfile::TempDir;
use tokio::sync::OnceCell;

struct TestContext {
    db: Arc<sea_orm::DatabaseConnection>,
    _temp_dir: TempDir,
}

static TEST_CONTEXT: OnceCell<TestContext> = OnceCell::const_new();

async fn test_context() -> &'static TestContext {
    TEST_CONTEXT
        .get_or_init(|| async {
            let db = Arc::new(common::setup_test_db_with_migrations().await);
            set_mcp_server_repository(SqliteMCPServerRepository::new((*db).clone()));
            set_session_repository(SqliteSessionRepository::new((*db).clone()));
            TestContext {
                db,
                _temp_dir: TempDir::new().expect("temp dir"),
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
