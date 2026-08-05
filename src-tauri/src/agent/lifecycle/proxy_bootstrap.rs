use crate::agent::runtime_state::SessionRuntimeState;
use crate::mcp::MCPServiceProxyManager;
use std::sync::Arc;
use tauri::AppHandle;

/// Kick off MCP proxy creation without blocking session open/create.
///
/// Session UI must return as soon as metadata is active. External MCP HTTP/stdio
/// work continues in the background; workflows soft-wait via `ensure_proxy_ready`.
pub async fn spawn_create_proxy_for_session(
    proxy_manager: Arc<MCPServiceProxyManager>,
    app_handle: AppHandle,
    session_id: String,
    tool_ids: Vec<String>,
    mcp_server_ids: Vec<String>,
) {
    // Surface an immediate hydrating state when no proxy is published yet so the
    // open response is not stuck on NotStarted while create_proxy is still queued.
    if proxy_manager.get_proxy(&session_id).await.is_none() {
        let current = proxy_manager.get_runtime_state(&session_id).await;
        if matches!(
            current.phase,
            crate::agent::runtime_state::SessionRuntimePhase::NotStarted
        ) {
            let _ = proxy_manager
                .set_runtime_state(
                    &session_id,
                    SessionRuntimeState::hydrating(),
                    Some(&app_handle),
                )
                .await;
        }
    }

    tokio::spawn(async move {
        match proxy_manager
            .create_proxy(
                session_id.clone(),
                tool_ids,
                mcp_server_ids,
                Some(app_handle.clone()),
            )
            .await
        {
            Ok(_) => {
                log::info!(
                    "Background MCP proxy creation finished for session: {}",
                    session_id
                );
            }
            Err(error) => {
                log::error!(
                    "Background MCP proxy creation failed for session {}: {}",
                    session_id,
                    error
                );
                let _ = proxy_manager
                    .set_runtime_state(
                        &session_id,
                        SessionRuntimeState::failed(error),
                        Some(&app_handle),
                    )
                    .await;
            }
        }
    });
}
