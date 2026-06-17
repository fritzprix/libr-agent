use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionStatus;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

pub async fn reset_session_execution_state(session: &mut AgentSession) {
    session.cancel_pending.store(false, Ordering::SeqCst);
    session.cancellation_token = CancellationToken::new();
    *session.repeated_thinking_retry_count.write().await = 0;
    *session.repeated_text_loop_retry_count.write().await = 0;
    // Safety valve: clear any stale in-flight compaction state before
    // explicitly starting or restarting a workflow from the current stack.
    session.compaction.clear_runtime_state(false).await;
}

pub async fn start_workflow(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
    user_message: Message,
) -> Result<(), String> {
    // Ensure the message cache is populated from DB before the dedup check.
    // Without this, an uninitialized (empty) cache would silently pass the duplicate
    // check, and the session would get stuck in Busy state when the second dedup
    // in append_user_message_to_session correctly rejects the duplicate after init.
    crate::agent::lifecycle::ensure_cache_initialized(active_sessions, &session_id).await?;

    // Note: we intentionally do NOT check is_cancelled() here.
    // cancel_workflow (soft cancel) leaves the token in a cancelled state to block
    // stale LLM responses, but start_workflow resets it unconditionally below.
    // Failing here would prevent users from sending new messages after a cancel.

    // Check status, deduplicate, and queue if busy (Atomic Check-and-Act)
    let should_queue = {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            // Deduplicate: Check if message ID already exists
            {
                let messages = session.messages.read().await;
                if messages.iter().any(|m| m.id == user_message.id) {
                    log::warn!(
                        "Ignoring duplicate message start_workflow: {}",
                        user_message.id
                    );
                    return Ok(());
                }
            }

            // Check Status
            let is_transitioning_to_busy = {
                let trans = session.status_transition.read().await;
                matches!(
                    trans.as_ref(),
                    Some(crate::agent::state::SessionStatusTransition::ToStatus(
                        SessionStatus::Busy | SessionStatus::Queued
                    ))
                )
            };

            if session.metadata.status == SessionStatus::Busy
                || session.metadata.status == SessionStatus::Queued
                || is_transitioning_to_busy
            {
                log::info!(
                    "Session {} is busy or queued. Queueing message: {} in pending_events only.",
                    session_id,
                    user_message.id
                );
                true // Signal that we queued it
            } else {
                false // Not busy/queued, proceed to start workflow
            }
        } else {
            false // Session not found, will be handled by standard flow (or fail there)
        }
    }; // Lock released here

    if should_queue {
        // Add to Pending Events and Persist to DB without touching session.messages
        // This prevents polluting the active context window mid-tool-execution
        crate::services::MessageService::queue_user_message(
            active_sessions,
            &session_id,
            &user_message,
        )
        .await?;

        // Do NOT call request_llm_completion. The existing busy workflow will pick it up.
        // Do NOT emit MessageAdded (it will be emitted when drained).
        return Ok(());
    }

    // Explicit new workflow start: reset all cancellation state.
    // Uses a write lock to also reset the cancellation_token, which cancel_workflow
    // may have left in a cancelled state to block stale LLM responses.
    {
        let mut active = active_sessions.write().await;
        if let Some(session) = active.get_mut(&session_id) {
            reset_session_execution_state(session).await;
        }
    }

    // --- STANDARD START WORKFLOW (Idle/Paused) ---

    // Update status to Queued immediately
    crate::agent::lifecycle::update_session_status(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Queued,
    )
    .await?;

    // Emit workflow started event
    let event = crate::agent::events::AgentEvent::WorkflowStarted {
        session_id: session_id.clone(),
    };
    log::info!("Emitting WorkflowStarted event for session: {}", session_id);
    if let Err(e) = crate::agent::tauri_events::emit_agent_event(app_handle, event) {
        log::error!("Failed to emit WorkflowStarted event: {}", e);
        return Err(format!("Failed to emit event: {}", e));
    }

    // Delegate message deduplication, cache update, DB insertion, and UI event emission
    // to the MessageService to maintain clean architectural boundaries.
    crate::services::MessageService::append_user_message_to_session(
        active_sessions,
        app_handle,
        &session_id,
        &user_message,
    )
    .await?;

    log::info!(
        "Queued workflow for session: {} with message: {}",
        session_id,
        user_message.id
    );

    let session_repo = Arc::clone(session_repo);
    let active_sessions = Arc::clone(active_sessions);
    let proxy_manager = Arc::clone(proxy_manager);
    let app_handle = app_handle.clone();
    let session_id_clone = session_id.clone();

    tokio::spawn(async move {
        // 1. Acquire active agent permit first (blocks safely outside transition lock)
        let gate = crate::state::get_concurrency_gate();
        let permit = match gate.acquire_active_agent().await {
            Ok(p) => p,
            Err(e) => {
                log::error!(
                    "Failed to acquire active agent permit for session {}: {}",
                    session_id_clone,
                    e
                );
                let error_event = crate::agent::events::AgentEvent::WorkflowError {
                    session_id: session_id_clone.clone(),
                    error: crate::agent::llm::types::AgentRuntimeError::new(
                        crate::agent::llm::types::AgentRuntimeErrorType::AiServiceError,
                        e.to_string(),
                    )
                    .with_code("BACKGROUND_WORKFLOW_FAILED"),
                };
                let _ = crate::agent::tauri_events::emit_agent_event(&app_handle, error_event);
                return;
            }
        };

        // 2. Put permit into memory and verify not cancelled/changed
        {
            let mut active = active_sessions.write().await;
            if let Some(session) = active.get_mut(&session_id_clone) {
                if session.cancellation_token.is_cancelled()
                    || session.metadata.status != SessionStatus::Queued
                {
                    log::info!(
                        "Session {} cancelled or status changed while queued. Aborting background start.",
                        session_id_clone
                    );
                    drop(permit);
                    return;
                }
                session.active_permit = Some(permit);
            } else {
                log::warn!(
                    "Session {} not found in active sessions during background start. Aborting.",
                    session_id_clone
                );
                drop(permit);
                return;
            }
        }

        // Transition status to Busy (bypasses concurrency gate block since permit is held)
        if let Err(e) = crate::agent::lifecycle::update_session_status(
            &session_repo,
            &active_sessions,
            &app_handle,
            &session_id_clone,
            SessionStatus::Busy,
        )
        .await
        {
            log::error!(
                "Failed to transition session {} to Busy: {}",
                session_id_clone,
                e
            );
            {
                let mut active = active_sessions.write().await;
                if let Some(session) = active.get_mut(&session_id_clone) {
                    session.active_permit.take();
                }
            }
            let error_event = crate::agent::events::AgentEvent::WorkflowError {
                session_id: session_id_clone.clone(),
                error: crate::agent::llm::types::AgentRuntimeError::new(
                    crate::agent::llm::types::AgentRuntimeErrorType::AiServiceError,
                    e.to_string(),
                )
                .with_code("BACKGROUND_WORKFLOW_FAILED"),
            };
            let _ = crate::agent::tauri_events::emit_agent_event(&app_handle, error_event);
            return;
        }

        if let Err(e) = ensure_proxy_ready(&proxy_manager, &app_handle, &session_id_clone, 60).await
        {
            log::error!(
                "Proxy check failed during background start for session {}: {}",
                session_id_clone,
                e
            );
            let _ = crate::agent::lifecycle::update_session_status(
                &session_repo,
                &active_sessions,
                &app_handle,
                &session_id_clone,
                SessionStatus::Error,
            )
            .await;
            let error_event = crate::agent::events::AgentEvent::WorkflowError {
                session_id: session_id_clone.clone(),
                error: crate::agent::llm::types::AgentRuntimeError::new(
                    crate::agent::llm::types::AgentRuntimeErrorType::AiServiceError,
                    e.to_string(),
                )
                .with_code("BACKGROUND_WORKFLOW_FAILED"),
            };
            let _ = crate::agent::tauri_events::emit_agent_event(&app_handle, error_event);
            return;
        }

        // Request LLM completion with cached messages
        if let Err(e) = crate::agent::llm::request_llm_completion_with_recovery(
            &session_repo,
            &active_sessions,
            &proxy_manager,
            &app_handle,
            session_id_clone.clone(),
        )
        .await
        {
            log::error!(
                "LLM completion failed in background start for session {}: {:?}",
                session_id_clone,
                e
            );
            let _ = crate::agent::lifecycle::update_session_status(
                &session_repo,
                &active_sessions,
                &app_handle,
                &session_id_clone,
                SessionStatus::Error,
            )
            .await;
            let error_event = crate::agent::events::AgentEvent::WorkflowError {
                session_id: session_id_clone.clone(),
                error: crate::agent::llm::types::AgentRuntimeError::new(
                    crate::agent::llm::types::AgentRuntimeErrorType::AiServiceError,
                    e.to_string(),
                )
                .with_code("BACKGROUND_WORKFLOW_FAILED"),
            };
            let _ = crate::agent::tauri_events::emit_agent_event(&app_handle, error_event);
        }
    });

    Ok(())
}

/// Helper to ensure a proxy exists for the session before invoking LLM
pub(crate) async fn ensure_proxy_exists(
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<(), String> {
    if proxy_manager.get_proxy(session_id).await.is_some() {
        return Ok(());
    }

    log::warn!(
        "MCP proxy missing for session {} during workflow start. Recreating...",
        session_id
    );

    match proxy_manager
        .ensure_configured_proxy(session_id, Some(app_handle.clone()))
        .await
    {
        Ok(_) => {
            log::info!(
                "Successfully ensured configured MCP proxy for session: {}",
                session_id
            );
        }
        Err(error)
            if error == format!("Session not found: {}", session_id)
                || error == "Session has no config" =>
        {
            log::error!(
                "Cannot recreate proxy during workflow start for session {}: {}",
                session_id,
                error
            );
        }
        Err(error) => return Err(error),
    }

    Ok(())
}

/// Ensure workflow execution never mistakes "missing proxy" for "ready proxy".
pub(crate) async fn ensure_proxy_ready(
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: &str,
    timeout_secs: u64,
) -> Result<(), String> {
    ensure_proxy_exists(proxy_manager, app_handle, session_id).await?;
    proxy_manager
        .wait_until_proxy_ready(session_id, timeout_secs)
        .await
}
