//! Pending-event continuation used by the finish-window guards.

use super::WorkflowRestart;
use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

pub async fn session_has_pending_events(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> bool {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        return session.pending_events.read().await.count() > 0;
    }

    false
}

/// Re-check `pending_events` immediately before transitioning to Idle.
///
/// Returns `true` when pending messages were found and a new LLM turn was
/// requested. Callers must skip the Idle transition in that case.
///
/// This is invoked twice on the success settlement path: once inside
/// [`settle_before_terminal_transition`] and again immediately before writing
/// `Idle`. Between those two scans another task can enqueue into
/// `pending_events` (the finish-window race). The second scan closes that
/// window without introducing a new terminal state machine.
pub async fn continue_workflow_if_pending_events(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    restart: &dyn WorkflowRestart,
    session_id: &str,
) -> Result<bool, String> {
    if !session_has_pending_events(active_sessions, session_id).await {
        return Ok(false);
    }

    log::info!(
        "Pending messages detected for session {} during workflow finish. Continuing workflow.",
        session_id
    );

    restart
        .restart(
            session_repo,
            active_sessions,
            proxy_manager,
            app_handle,
            session_id.to_string(),
        )
        .await?;

    Ok(true)
}
