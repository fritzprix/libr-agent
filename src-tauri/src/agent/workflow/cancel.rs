use crate::agent::llm::types::CompletionCancelRequest;
use crate::agent::state::AgentSession;
use crate::agent::tauri_events::emit_completion_cancel;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionStatus;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelStrategy {
    DeferToMessageBoundary,
    StopImmediately,
}

pub fn classify_cancel_strategy(has_pending_execution: bool) -> CancelStrategy {
    if has_pending_execution {
        CancelStrategy::DeferToMessageBoundary
    } else {
        CancelStrategy::StopImmediately
    }
}

pub fn should_consume_cancel_at_message_boundary(cancel_pending: bool) -> bool {
    cancel_pending
}

/// Cancel is a no-op when the session is already inactive — no workflow to stop.
pub fn is_inactive_cancel_noop(status: &SessionStatus) -> bool {
    matches!(
        status,
        SessionStatus::Idle | SessionStatus::Paused | SessionStatus::Error
    )
}

/// Abort any in-flight frontend LLM completion for this session.
///
/// Status transitions alone must not cancel from the React side — that races with
/// legitimate turns. Cancel/terminate own the abort via `llm:completion-cancel`.
async fn cancel_frontend_completion_if_pending(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    reason: &str,
) {
    let response_message_id = {
        let active = active_sessions.read().await;
        let Some(session) = active.get(session_id) else {
            return;
        };
        let mut expected = session.expected_response_id.write().await;
        expected.take()
    };

    let Some(response_message_id) = response_message_id else {
        return;
    };

    if let Err(error) = emit_completion_cancel(
        app_handle,
        CompletionCancelRequest {
            session_id: session_id.to_string(),
            response_message_id,
            reason: reason.to_string(),
        },
    ) {
        log::warn!(
            "Failed to emit llm:completion-cancel for session {}: {}",
            session_id,
            error
        );
    }
}

/// This triggers the cancellation token to abort any running operations
pub async fn terminate_session(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    log::info!("Terminating workflow for session: {}", session_id);

    // Abort frontend LLM first so late chunks cannot keep ThinkingBubble alive
    // after we flip the session to idle.
    cancel_frontend_completion_if_pending(
        active_sessions,
        app_handle,
        &session_id,
        "workflow-terminated",
    )
    .await;

    // 1. Trigger cancellation token if the session is active in memory
    let session_active = {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            session.cancel_pending.store(true, Ordering::SeqCst);
            session.cancellation_token.cancel();
            true
        } else {
            false
        }
    };

    // 2. Destroy proxy for this session - ALWAYS do this!
    // This prevents resource leaks (MCP processes) even if the session wasn't active
    // (e.g. failed resume or already closed in memory but proxy still exists).
    proxy_manager.destroy_proxy(&session_id).await;
    log::info!("Destroyed MCP proxy for session: {}", session_id);

    // 3. Update status to idle in DB (and memory if active)
    // We ignore error here for inactive sessions to ensure best-effort cleanup
    let _ = crate::agent::lifecycle::update_session_status(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Idle,
    )
    .await;

    // 4. Cleanup memory state if active
    if session_active {
        let mut active = active_sessions.write().await;
        if let Some(session) = active.get_mut(&session_id) {
            // Reset cancellation state so a later inject/start_workflow on this
            // session is not treated as still cancelled.
            session.cancel_pending.store(false, Ordering::SeqCst);
            session.cancellation_token = CancellationToken::new();
        }
    }

    // Hard terminate drops waiting prompts; soft cancel preserves them.
    discard_pending_events(active_sessions, &session_id).await;

    // 5. Emit workflow stopped event
    let event = crate::agent::events::AgentEvent::WorkflowCompleted {
        session_id: session_id.clone(),
        reason: crate::agent::events::WorkflowCompletionReason::Terminated,
    };
    crate::agent::tauri_events::emit_agent_event(app_handle, event)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    log::info!("Terminated workflow for session: {}", session_id);

    if !session_active {
        // Return error for non-active sessions to maintain backward compatibility with callers
        // that expect to know if the session was not in memory, but AFTER cleanup.
        return Err(format!("Session not found: {}", session_id));
    }

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
    let (has_pending_execution, current_status) = {
        let active = active_sessions.read().await;
        let session = active
            .get(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        (
            session.pending_execution.is_some(),
            session.metadata.status.clone(),
        )
    };

    // Idle/Paused/Error: nothing to cancel. Still abort a stale frontend LLM
    // completion if one is registered, but do not force a Paused transition or
    // contend on status locks — that races with `/clear` (reset_session) and
    // falsely leaves Cancel UI armed while the session is already inactive.
    if is_inactive_cancel_noop(&current_status) {
        cancel_frontend_completion_if_pending(
            active_sessions,
            app_handle,
            &session_id,
            "workflow-cancel-noop-inactive",
        )
        .await;
        log::info!(
            "Cancel requested for session {} while {:?} — no running workflow to stop",
            session_id,
            current_status
        );
        return Ok(());
    }

    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            session.cancel_pending.store(true, Ordering::SeqCst);
        }
    }

    if classify_cancel_strategy(has_pending_execution) == CancelStrategy::DeferToMessageBoundary {
        log::info!(
            "Cancel requested for session {} (deferred to message boundary)",
            session_id
        );
        // Waiting prompts stay in the durable FIFO queue so the user can
        // cancel them individually or resume later.
        // SP6: Wake any awaitAgent/pollProcess waiter that is suspended inside
        // a tool call for THIS session. The deferred cancel only sets
        // cancel_pending; without this notification the waiter would sleep up
        // to 30 s before re-checking the flag.
        crate::state::get_session_bus().notify_status_change(&session_id);
        return Ok(());
    }

    // No in-flight tool-call batch: stop immediately and leave the workflow paused
    // so the user can explicitly resume from the current stack.
    cancel_frontend_completion_if_pending(
        active_sessions,
        app_handle,
        &session_id,
        "workflow-cancelled",
    )
    .await;

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
        session.cancel_pending.store(false, Ordering::SeqCst);
        // Cancel the token (do NOT replace with a fresh one yet).
        // The cancelled state persists until start_workflow explicitly resets it.
        // This prevents stale in-flight LLM responses carrying tool_calls from
        // re-entering the workflow via the Idle+allow_idle_tool_entry path after cancel.
        session.cancellation_token.cancel();
    }
    drop(active);

    let event = crate::agent::events::AgentEvent::WorkflowCompleted {
        session_id: session_id.clone(),
        reason: crate::agent::events::WorkflowCompletionReason::Cancelled,
    };
    crate::agent::tauri_events::emit_agent_event(app_handle, event)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    log::info!("Cancelled workflow for session: {}", session_id);
    Ok(())
}

pub(crate) async fn discard_pending_events(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) {
    if let Err(e) =
        crate::agent::pending_queue::discard_all_pending_messages(active_sessions, None, session_id)
            .await
    {
        log::error!(
            "Failed to discard pending messages for session {}: {}",
            session_id,
            e
        );
    }
}
