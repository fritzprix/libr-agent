mod common;

use std::sync::Arc;

use tokio::sync::OnceCell;

use tauri_mcp_agent_lib::agent::tools::extract_builtin_tool_ids;
use tauri_mcp_agent_lib::agent::AgentConfig;
use tauri_mcp_agent_lib::mcp::MCPServiceProxyManager;
use tauri_mcp_agent_lib::repositories::{
    MCPServerRepository, SessionMetadata, SessionRepository, SessionStatus,
    SqliteMCPServerRepository, SqliteSessionRepository,
};
use tauri_mcp_agent_lib::session::SessionManager;
use tauri_mcp_agent_lib::{set_mcp_server_repository, set_session_repository};

static TEST_DB: OnceCell<sea_orm::DatabaseConnection> = OnceCell::const_new();

async fn ensure_test_state() -> sea_orm::DatabaseConnection {
    TEST_DB
        .get_or_init(|| async {
            let db = common::setup_test_db_with_migrations().await;
            set_session_repository(SqliteSessionRepository::new(db.clone()));
            set_mcp_server_repository(SqliteMCPServerRepository::new(db.clone()));
            db
        })
        .await
        .clone()
}

fn build_session_metadata(session_id: &str, agent_config: &AgentConfig) -> SessionMetadata {
    let now = chrono::Utc::now().timestamp_millis();

    SessionMetadata {
        id: session_id.to_string(),
        name: Some("Resume proxy regression".to_string()),
        status: SessionStatus::Idle,
        model: "gpt-5.4".to_string(),
        provider: "openai".to_string(),
        agent_config: Some(
            agent_config
                .to_json()
                .expect("agent config should serialize"),
        ),
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
        yolo_mode: false,
        workspace_override: None,
    }
}

#[tokio::test]
async fn create_proxy_replaces_lazy_builtin_only_proxy_when_external_servers_are_requested() {
    let db = ensure_test_state().await;
    let session_repo = SqliteSessionRepository::new(db.clone());
    let mcp_repo = SqliteMCPServerRepository::new(db.clone());

    let server = mcp_repo
        .create(
            "exa",
            serde_json::json!({
                "transport": {
                    "type": "http",
                    "url": "http://127.0.0.1:9/mcp"
                }
            }),
        )
        .await
        .expect("MCP server config should be created");

    let agent_config = AgentConfig {
        id: Some("resume-proxy-assistant".to_string()),
        name: "Resume proxy regression".to_string(),
        system_prompt: "You verify proxy recreation.".to_string(),
        mcp_server_ids: vec![server.id.clone()],
        allowed_built_in_service_aliases: Some(vec![]),
        ..Default::default()
    };
    let session_id = format!("resume-proxy-{}", uuid::Uuid::new_v4());

    session_repo
        .upsert_session(&build_session_metadata(&session_id, &agent_config))
        .await
        .expect("session should be stored");

    let manager = Arc::new(MCPServiceProxyManager::new(
        Arc::new(db.clone()),
        Arc::new(SessionManager::new().expect("session manager should initialize")),
    ));

    let lazy_proxy = manager
        .ensure_builtin_proxy(&session_id)
        .await
        .expect("lazy builtin proxy should initialize");
    assert!(
        lazy_proxy
            .configured_external_server_names()
            .await
            .is_empty(),
        "lazy proxy must start builtin-only"
    );

    let rebuilt_proxy = manager
        .create_proxy(
            session_id.clone(),
            extract_builtin_tool_ids(&agent_config),
            agent_config.mcp_server_ids.clone(),
            None,
        )
        .await
        .expect("resume/create proxy should rebuild proxy with external server config");

    assert_eq!(
        rebuilt_proxy.configured_external_server_names().await,
        vec!["exa".to_string()],
        "create_proxy must replace builtin-only lazy proxies when the session config includes external MCP servers"
    );
}
