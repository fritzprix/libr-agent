use super::AgentSessionManager;
use crate::agent::events::AgentEventDispatcher;
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::models::chat::Message;
use crate::repositories::SessionStatus;
use std::sync::Arc;

pub async fn inject_messages(
    manager: &AgentSessionManager,
    session_id: String,
    messages: Vec<Message>,
) -> Result<bool, String> {
    let should_trigger_workflow = {
        let sessions = manager.active_sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        let is_transitioning_to_busy_or_queued = {
            let trans = session.status_transition.read().await;
            matches!(
                trans.as_ref(),
                Some(crate::agent::state::SessionStatusTransition::ToStatus(
                    SessionStatus::Busy | SessionStatus::Queued
                ))
            )
        };
        // Compaction in flight (Manual or Preflight): queue into pending_events.
        // Starting a workflow here would reset_session_execution_state and clear
        // the in-flight compact.
        let compaction_in_flight = session.compaction.snapshot().await.is_in_flight();

        session.metadata.status != SessionStatus::Busy
            && session.metadata.status != SessionStatus::Queued
            && !is_transitioning_to_busy_or_queued
            && !compaction_in_flight
            // A force-terminated session accepts automatic messages (channel
            // notifications, scheduled callbacks, agent-tool follow-ups) into
            // its history, but they must never start a workflow. Only an
            // explicit user/API entry clears the fence first, and that entry
            // has already passed the fence check by the time it gets here
            // (issue #2633 G2.3).
            && !manager.proxy_manager.is_teardown_fenced(&session_id).await
    };

    // Delegate message persistence, caching, and event emission to MessageService
    crate::services::MessageService::inject_messages_to_session(
        &manager.active_sessions,
        &manager.app_handle,
        &session_id,
        messages,
        should_trigger_workflow,
    )
    .await?;

    if should_trigger_workflow {
        log::info!(
            "Triggering workflow after message injection for session: {}",
            session_id
        );

        {
            let mut sessions = manager.active_sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                crate::agent::workflow::start::reset_session_execution_state(session).await;
            }
        }

        // Update status to Queued immediately
        crate::agent::lifecycle::update_session_status(
            &manager.session_repo,
            &manager.active_sessions,
            &manager.app_handle,
            &session_id,
            crate::repositories::SessionStatus::Queued,
        )
        .await?;

        // Emit workflow started event
        let event = crate::agent::events::AgentEvent::WorkflowStarted {
            session_id: session_id.clone(),
        };
        if let Err(e) =
            TauriEventDispatcher::from_handle(&manager.app_handle).emit_agent_event(event)
        {
            log::error!(
                "Failed to emit WorkflowStarted event during injection: {}",
                e
            );
        }

        let session_repo = Arc::clone(&manager.session_repo);
        let active_sessions = Arc::clone(&manager.active_sessions);
        let proxy_manager = Arc::clone(&manager.proxy_manager);
        let app_handle = manager.app_handle.clone();
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
                        .with_code("BACKGROUND_INJECTION_FAILED"),
                    };
                    let _ = TauriEventDispatcher::from_handle(&app_handle)
                        .emit_agent_event(error_event);
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
                            "Session {} cancelled or status changed while queued. Aborting background injection.",
                            session_id_clone
                        );
                        drop(permit);
                        return;
                    }
                    session.active_permit = Some(permit);
                } else {
                    log::warn!(
                        "Session {} not found in active sessions during background injection. Aborting.",
                        session_id_clone
                    );
                    drop(permit);
                    return;
                }
            }

            // Transition status to Busy (bypasses concurrency gate block since permit is held)
            let promoted = crate::agent::lifecycle::promote_queued_session_to_busy(
                &session_repo,
                &active_sessions,
                &app_handle,
                &session_id_clone,
            )
            .await;
            if let Ok(false) = promoted {
                log::info!(
                    "Session {} left Queued before its injected workflow started. Not starting it.",
                    session_id_clone
                );
                return;
            }
            if let Err(e) = promoted {
                log::error!(
                    "Failed to transition injected session {} to Busy: {}",
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
                    .with_code("BACKGROUND_INJECTION_FAILED"),
                };
                let _ =
                    TauriEventDispatcher::from_handle(&app_handle).emit_agent_event(error_event);
                return;
            }

            // Ensure proxy is ready
            let proxy_ready_timeout = proxy_manager.startup_timeout_secs().await;
            if let Err(e) = crate::agent::workflow::start::ensure_proxy_ready(
                &proxy_manager,
                &app_handle,
                &session_id_clone,
                proxy_ready_timeout,
            )
            .await
            {
                if crate::mcp::MCPServiceProxyManager::is_teardown_fence_rejection(&e) {
                    // A force-stop fence refusal is a correct outcome, not a failure:
                    // undo the Busy promotion and emit no WorkflowError (#2633 G4.1).
                    log::info!(
                        "Background injection for session {} aborted by force-stop fence; no workflow error emitted",
                        session_id_clone
                    );
                    if let Err(err) = crate::agent::lifecycle::end_fence_refused_wake(
                        &session_repo,
                        &active_sessions,
                        &app_handle,
                        &session_id_clone,
                    )
                    .await
                    {
                        log::warn!(
                            "Failed to end refused background injection for session {}: {}",
                            session_id_clone,
                            err
                        );
                    }
                    return;
                }

                log::error!(
                    "Proxy check failed during background injection for session {}: {}",
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
                    .with_code("BACKGROUND_INJECTION_FAILED"),
                };
                let _ =
                    TauriEventDispatcher::from_handle(&app_handle).emit_agent_event(error_event);
                return;
            }

            // Trigger LLM to pick up where it left off
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
                    "LLM completion failed in background injection for session {}: {:?}",
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
                    .with_code("BACKGROUND_INJECTION_FAILED"),
                };
                let _ =
                    TauriEventDispatcher::from_handle(&app_handle).emit_agent_event(error_event);
            }
        });
    }

    Ok(should_trigger_workflow)
}

impl AgentSessionManager {
    /// Inject messages into the session
    pub async fn inject_messages(
        &self,
        session_id: String,
        messages: Vec<Message>,
    ) -> Result<bool, String> {
        self.ensure_session_active(&session_id).await?;
        inject_messages(self, session_id, messages).await
    }

    /// Append messages into session history without triggering a workflow or touching pending_queue
    pub async fn append_messages(
        &self,
        session_id: &str,
        messages: Vec<Message>,
    ) -> Result<(), String> {
        self.ensure_session_active(session_id).await?;
        crate::services::MessageService::append_messages_without_workflow(
            &self.active_sessions,
            &self.app_handle,
            session_id,
            messages,
        )
        .await
    }

    pub async fn get_pending_queue(&self, session_id: &str) -> Result<Vec<Message>, String> {
        self.ensure_session_active(session_id).await?;
        crate::agent::pending_queue::list_pending_messages(&self.active_sessions, session_id).await
    }

    pub async fn cancel_pending_prompt(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<bool, String> {
        self.ensure_session_active(session_id).await?;
        crate::agent::pending_queue::cancel_pending_message(
            &self.active_sessions,
            &self.app_handle,
            session_id,
            message_id,
        )
        .await
    }

    /// Remove a message from the in-memory cache.
    /// Used when messages are deleted via messages_delete command to keep cache in sync.
    pub async fn remove_message_from_cache(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<(), String> {
        let sessions = self.active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            let mut messages = session.messages.write().await;
            messages.retain(|m| m.id != message_id);
            log::debug!(
                "Removed message {} from in-memory cache for session {}. Remaining: {}",
                message_id,
                session_id,
                messages.len()
            );
            Ok(())
        } else {
            Ok(())
        }
    }
}
