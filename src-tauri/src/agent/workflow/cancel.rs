use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::{MessageRepository, SessionStatus};
use std::collections::{HashMap, HashSet};
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
/// This triggers the cancellation token to abort any running operations
pub async fn terminate_session(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
) -> Result<(), String> {
    log::info!("Terminating workflow for session: {}", session_id);

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
            // Reset cancellation token for potential future workflows
            session.cancellation_token = CancellationToken::new();
        }
    }

    // 5. Emit workflow stopped event
    let event = crate::agent::events::AgentEvent::WorkflowCompleted {
        session_id: session_id.clone(),
        reason: crate::agent::events::WorkflowCompletionReason::Cancelled,
    };
    crate::agent::events::emit_agent_event(app_handle, event)
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
        // SP6: Wake any awaitAgent/pollProcess waiter that is suspended inside
        // a tool call for THIS session. The deferred cancel only sets
        // cancel_pending; without this notification the waiter would sleep up
        // to 30 s before re-checking the flag.
        crate::state::get_session_bus().notify_status_change(&session_id);
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
        reason: crate::agent::events::WorkflowCompletionReason::Cancelled,
    };
    crate::agent::events::emit_agent_event(app_handle, event)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    log::info!("Cancelled workflow for session: {}", session_id);
    Ok(())
}

pub(crate) async fn discard_pending_events(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) {
    let mut messages_to_delete = Vec::new();

    // 1. Drain from pending events queue and remove from in-memory cache
    {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            let mut pending_events = session.pending_events.write().await;
            messages_to_delete = pending_events.drain_messages();

            if !messages_to_delete.is_empty() {
                let mut messages = session.messages.write().await;
                // SP2: Convert to HashSet for O(1) lookups during retain (was O(n*m))
                let delete_set: HashSet<String> = messages_to_delete.iter().cloned().collect();
                // Remove these messages from the cache
                messages.retain(|m| !delete_set.contains(&m.id));

                log::info!(
                    "Cleared {} pending events from queue and cache for session {}",
                    messages_to_delete.len(),
                    session_id
                );
            }
        }
    }

    // 2. Delete from database
    if !messages_to_delete.is_empty() {
        let repo = crate::state::get_message_repository();
        for msg_id in messages_to_delete {
            if let Err(e) = repo.delete_by_id(&msg_id).await {
                log::error!(
                    "Failed to delete cancelled pending message {} from DB: {}",
                    msg_id,
                    e
                );
            }
        }
    }
}
