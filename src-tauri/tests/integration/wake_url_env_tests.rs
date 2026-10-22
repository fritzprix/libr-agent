use crate::common;

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::agent::concurrency::{
    ConcurrencyGate, DEFAULT_MAX_ACTIVE_AGENTS, DEFAULT_MAX_ACTIVE_PROCESSES,
    DEFAULT_MAX_SUSPENDED_AGENTS, DEFAULT_MAX_SUSPENDED_PROCESSES,
};
use tauri_mcp_agent_lib::agent::session_bus::SessionBus;
use tauri_mcp_agent_lib::lifecycle::repositories::init_repositories;
use tauri_mcp_agent_lib::mcp::builtin::workspace::{WorkspaceServer, RUN_SHELL_TOOL};
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
use tauri_mcp_agent_lib::server::wake::{revoke_wake_token, wake_token_for};
use tauri_mcp_agent_lib::session::SessionManager;
use tauri_mcp_agent_lib::{init_concurrency_gate, init_session_bus};
use tempfile::tempdir;
use tokio::sync::OnceCell;

fn extract_text_content(result: &MCPResult) -> String {
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

async fn ensure_repositories() {
    static REPOSITORIES: OnceCell<()> = OnceCell::const_new();

    REPOSITORIES
        .get_or_init(|| async {
            let db = common::setup_test_db_with_migrations().await;
            init_repositories(&db).await;
            init_session_bus(SessionBus::new());
            init_concurrency_gate(ConcurrencyGate::new(
                DEFAULT_MAX_ACTIVE_AGENTS,
                DEFAULT_MAX_SUSPENDED_AGENTS,
                DEFAULT_MAX_ACTIVE_PROCESSES,
                DEFAULT_MAX_SUSPENDED_PROCESSES,
            ));
        })
        .await;
}

#[cfg(unix)]
#[tokio::test]
async fn run_shell_exports_session_wake_url_without_echoing_token() {
    ensure_repositories().await;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "wake-url-env-test";
    let session_manager =
        SessionManager::new_with_base_dir(temp_dir.path().to_path_buf()).expect("session manager");
    let server = WorkspaceServer::new(session_id.to_string(), Arc::new(session_manager));

    let result = server
        .handle_run_shell(
            json!({
                "command": format!(
                    "case \"$LIBRAGENT_WAKE_URL\" in \
                     http://127.0.0.1:*/api/sessions/{session_id}/wake/*) echo WAKE_OK;; \
                     *) echo WAKE_MISSING;; esac"
                )
            }),
            session_id,
            RUN_SHELL_TOOL,
        )
        .await
        .expect("runShell should succeed");

    let text = extract_text_content(&result);
    let token = wake_token_for(session_id);
    assert!(text.contains("WAKE_OK"), "wake URL not exported: {text}");
    assert!(
        !text.contains(&token),
        "tool result must not contain the wake token"
    );
    revoke_wake_token(session_id);
}
