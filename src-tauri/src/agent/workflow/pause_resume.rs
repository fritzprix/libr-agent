use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionStatus;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
/// Pause a running workflow
pub async fn pause_workflow(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    crate::agent::lifecycle::update_session_status(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Paused,
    )
    .await?;

    log::info!("Paused workflow for session: {}", session_id);
    Ok(())
}

/// Resume a paused workflow
pub async fn resume_workflow(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    {
        let mut active = active_sessions.write().await;
        if let Some(session) = active.get_mut(&session_id) {
            crate::agent::workflow::start::reset_session_execution_state(session).await;
        }
    }

    // Ensure cache is initialized before resuming (lazy load if needed, preserve if exists)
    crate::agent::lifecycle::ensure_cache_initialized(active_sessions, &session_id).await?;

    crate::agent::lifecycle::update_session_status(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Busy,
    )
    .await?;

    log::info!("Resumed workflow status for session: {}", session_id);

    crate::agent::workflow::start::ensure_proxy_ready(proxy_manager, app_handle, &session_id, 60)
        .await?;

    // Trigger LLM to pick up where it left off
    crate::agent::llm::request_llm_completion(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id,
    )
    .await?;

    Ok(())
}
