use crate::agent::state::AgentSession;
use crate::agent::workflow::cancellation::{
    classify_cancel_strategy, discard_pending_events, CancelStrategy,
};
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionStatus;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

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

    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(&session_id) {
        session.is_running = false;
    }

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
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            session.cancel_pending.store(false, Ordering::SeqCst);
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

    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(&session_id) {
        session.is_running = true;
    }
    drop(active); // Drop lock before async call

    log::info!("Resumed workflow status for session: {}", session_id);

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

/// Terminate a running workflow
/// This triggers the cancellation token to abort any running operations
pub async fn terminate_session(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    log::info!("Terminating workflow for session: {}", session_id);

    // Trigger cancellation token to abort running loops
    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            session.cancel_pending.store(true, Ordering::SeqCst);
            session.cancellation_token.cancel();
        } else {
            return Err(format!("Session not found: {}", session_id));
        }
    }

    // Update status to idle (workflow stopped)
    crate::agent::lifecycle::update_session_status(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Idle,
    )
    .await?;

    // Destroy proxy for this session
    proxy_manager.destroy_proxy(&session_id).await;
    log::info!("Destroyed MCP proxy for session: {}", session_id);

    // Remove from active sessions and create a new cancellation token for future use
    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(&session_id) {
        session.is_running = false;
        // Reset cancellation token for potential future workflows
        session.cancellation_token = CancellationToken::new();
    }

    // Emit workflow stopped event
    let event = crate::agent::events::AgentEvent::WorkflowCompleted {
        session_id: session_id.clone(),
    };
    crate::agent::events::emit_agent_event(app_handle, event)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    log::info!("Terminated workflow for session: {}", session_id);
    Ok(())
}

/// Cancel a running workflow
/// This triggers the cancellation token to abort any running operations
pub async fn cancel_workflow(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    _proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    log::info!("Cancelling workflow for session: {}", session_id);

    // Determine whether to stop immediately or defer to message boundary.
    // If a tool-call batch is in progress, we only set cancel_pending and let
    // continue_workflow_after_tool consume it after the full message completes.
    let has_pending_execution = {
        let active = active_sessions.read().await;
        let session = active
            .get(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        session.cancel_pending.store(true, Ordering::SeqCst);
        session.pending_execution.is_some()
    };

    if classify_cancel_strategy(has_pending_execution) == CancelStrategy::DeferToMessageBoundary {
        log::info!(
            "Cancel requested for session {} (deferred to message boundary)",
            session_id
        );
        // Discard any user messages that arrived while the agent was busy and
        // are waiting in the pending_events queue. They must not be processed
        // after the agent stops, regardless of when the active tool batch
        // completes. The tool batch itself is still allowed to finish cleanly.
        discard_pending_events(active_sessions, &session_id).await;
        return Ok(());
    }

    // No in-flight tool-call batch: stop immediately.
    crate::agent::lifecycle::update_session_status(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Idle,
    )
    .await?;

    let mut active = active_sessions.write().await;
    if let Some(session) = active.get_mut(&session_id) {
        session.is_running = false;
        session.cancel_pending.store(false, Ordering::SeqCst);
        // Cancel the token (do NOT replace with a fresh one yet).
        // The cancelled state persists until start_workflow explicitly resets it.
        // This prevents stale in-flight LLM responses carrying tool_calls from
        // re-entering the workflow via the Idle+allow_idle_tool_entry path after cancel.
        session.cancellation_token.cancel();
    }
    drop(active);

    discard_pending_events(active_sessions, &session_id).await;

    let event = crate::agent::events::AgentEvent::WorkflowCompleted {
        session_id: session_id.clone(),
    };
    crate::agent::events::emit_agent_event(app_handle, event)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    log::info!("Cancelled workflow for session: {}", session_id);
    Ok(())
}
