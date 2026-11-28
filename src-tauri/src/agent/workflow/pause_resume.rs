use crate::agent::concurrency::{acquire_queued_permit, QueuedPermitError};
use crate::agent::events::AgentEventDispatcher;
use crate::agent::state::AgentSession;
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
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

/// True when the transcript ends with a completed assistant turn, i.e. an
/// explicit resume has nothing left to continue (issue #2898). An assistant
/// message still carrying `tool_calls` is an unanswered turn, not a completed
/// one, so it does not count.
fn has_completed_assistant_tail(messages: &[Message]) -> bool {
    matches!(
        messages.last(),
        Some(last)
            if last.role == "assistant"
                && last.is_streaming != Some(true)
                && last.tool_calls.as_deref().unwrap_or_default().is_empty()
    )
}

/// Resume a paused workflow
///
/// An `Idle` session whose transcript already ends with a completed assistant
/// turn has nothing to continue: the resume settles back to `Idle` without
/// starting an LLM turn (issue #2898). The caller is still responsible for
/// reloading the runtime (`resume_session`).
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
            if session.compaction.snapshot().await.is_in_flight() {
                return Err(format!(
                    "Cannot resume workflow for session {} while compaction is in flight",
                    session_id
                ));
            }
        }
    }

    // Load the transcript before evaluating the resume guard (issue #2898):
    // a cold session (e.g. after an app restart) has no in-memory messages
    // yet, and the guard must see the persisted tail. Preserve if initialized.
    crate::agent::lifecycle::ensure_cache_initialized(active_sessions, &session_id).await?;

    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            // Issue #2898: resuming an Idle session whose transcript already
            // ends with a completed assistant turn has nothing to continue.
            // Starting an LLM turn on the unchanged stack can fail (e.g.
            // thinking-only retries) and settle the session in `Error` with no
            // explanation. The caller already reloaded the runtime, so settle
            // back to Idle instead of starting a turn.
            if session.metadata.status == SessionStatus::Idle
                && session.pending_approvals.read().await.is_empty()
                && has_completed_assistant_tail(&session.messages.read().await)
            {
                log::info!(
                    "Nothing to resume for session {}: idle with a completed assistant turn; staying Idle",
                    session_id
                );
                return Ok(());
            }
        }
    }

    {
        let mut active = active_sessions.write().await;
        if let Some(session) = active.get_mut(&session_id) {
            crate::agent::workflow::start::reset_session_execution_state(session).await;
        }
    }

    // Transition status to Queued immediately
    crate::agent::lifecycle::update_session_status(
        session_repo,
        active_sessions,
        app_handle,
        &session_id,
        SessionStatus::Queued,
    )
    .await?;

    log::info!("Queued workflow resume for session: {}", session_id);

    let session_repo = Arc::clone(session_repo);
    let active_sessions = Arc::clone(active_sessions);
    let proxy_manager = Arc::clone(proxy_manager);
    let app_handle = app_handle.clone();
    let session_id_clone = session_id.clone();

    tokio::spawn(async move {
        // 1. Acquire active agent permit first (blocks safely outside transition
        //    lock). One shared queued-acquire helper (#3086 G1.4).
        let permit = match acquire_queued_permit(
            crate::state::get_concurrency_gate(),
            &active_sessions,
            &session_id_clone,
        )
        .await
        {
            Ok(p) => p,
            Err(QueuedPermitError::Cancelled) => {
                log::info!(
                    "Session {} cancelled while waiting in concurrency queue. Aborting background resume.",
                    session_id_clone
                );
                return;
            }
            Err(QueuedPermitError::MissingSession) => {
                log::warn!(
                    "Session {} disappeared while waiting in concurrency queue. Aborting background resume.",
                    session_id_clone
                );
                return;
            }
            Err(QueuedPermitError::Gate(e)) => {
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
                    .with_code("BACKGROUND_RESUME_FAILED"),
                };
                let _ =
                    TauriEventDispatcher::from_handle(&app_handle).emit_agent_event(error_event);
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
                        "Session {} cancelled or status changed while queued. Aborting background resume.",
                        session_id_clone
                    );
                    drop(permit);
                    return;
                }
                session.active_permit = Some(permit);
            } else {
                log::warn!(
                    "Session {} not found in active sessions during background resume. Aborting.",
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
                "Session {} left Queued before its resumed workflow started. Not starting it.",
                session_id_clone
            );
            return;
        }
        if let Err(e) = promoted {
            log::error!(
                "Failed to transition resumed session {} to Busy: {}",
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
                .with_code("BACKGROUND_RESUME_FAILED"),
            };
            let _ = TauriEventDispatcher::from_handle(&app_handle).emit_agent_event(error_event);
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
            // A force-stop fence refusal is a correct outcome, not a failure:
            // undo the Busy promotion and emit no WorkflowError (#2633 G4.1).
            if crate::mcp::MCPServiceProxyManager::is_teardown_fence_rejection(&e) {
                log::info!(
                    "Background resume for session {} aborted by force-stop fence; no workflow error emitted",
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
                        "Failed to end refused background resume for session {}: {}",
                        session_id_clone,
                        err
                    );
                }
                return;
            }
            log::error!(
                "Proxy check failed during background resume for session {}: {}",
                session_id_clone,
                e
            );
            let error = crate::agent::llm::types::AgentRuntimeError::new(
                crate::agent::llm::types::AgentRuntimeErrorType::AiServiceError,
                e.to_string(),
            )
            .with_code("BACKGROUND_RESUME_FAILED");
            let _ = crate::agent::workflow::finish::finalize_session_error(
                &session_repo,
                &active_sessions,
                &app_handle,
                &session_id_clone,
                error,
            )
            .await;
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
                "LLM completion failed in background resume for session {}: {:?}",
                session_id_clone,
                e
            );
            let error = crate::agent::llm::types::AgentRuntimeError::new(
                crate::agent::llm::types::AgentRuntimeErrorType::AiServiceError,
                e.to_string(),
            )
            .with_code("BACKGROUND_RESUME_FAILED");
            let _ = crate::agent::workflow::finish::finalize_session_error(
                &session_repo,
                &active_sessions,
                &app_handle,
                &session_id_clone,
                error,
            )
            .await;
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::{ToolCall, ToolCallFunction};
    use crate::models::chat::Message;

    fn msg(id: &str, role: &str, is_streaming: Option<bool>) -> Message {
        Message {
            id: id.to_string(),
            session_id: "s1".to_string(),
            role: role.to_string(),
            content: vec![],
            tool_calls: None,
            tool_call_id: None,
            is_streaming,
            thinking: None,
            thinking_signature: None,
            assistant_id: None,
            attachments: None,
            tool_use: None,
            usage: None,
            prompt_tokens: None,
            created_at: 1,
            updated_at: 1,
            source: None,
            error: None,
            metadata: None,
        }
    }

    #[test]
    fn completed_assistant_tail_detects_finished_turn() {
        let messages = vec![msg("u1", "user", Some(false)), msg("a1", "assistant", None)];
        assert!(has_completed_assistant_tail(&messages));
    }

    #[test]
    fn completed_assistant_tail_rejects_streaming_assistant() {
        let messages = vec![
            msg("u1", "user", Some(false)),
            msg("a1", "assistant", Some(true)),
        ];
        assert!(!has_completed_assistant_tail(&messages));
    }

    #[test]
    fn completed_assistant_tail_rejects_pending_user_message() {
        let messages = vec![msg("u1", "user", Some(false))];
        assert!(!has_completed_assistant_tail(&messages));
    }

    #[test]
    fn completed_assistant_tail_rejects_tool_tail() {
        let messages = vec![msg("a1", "assistant", Some(false)), msg("t1", "tool", None)];
        assert!(!has_completed_assistant_tail(&messages));
    }

    #[test]
    fn completed_assistant_tail_rejects_unanswered_tool_calls() {
        let mut tail = msg("a1", "assistant", Some(false));
        tail.tool_calls = Some(vec![ToolCall {
            id: "call-1".to_string(),
            function: ToolCallFunction {
                name: "example__tool".to_string(),
                arguments: "{}".to_string(),
            },
            r#type: "function".to_string(),
        }]);
        let messages = vec![msg("u1", "user", Some(false)), tail];
        assert!(!has_completed_assistant_tail(&messages));
    }

    #[test]
    fn completed_assistant_tail_accepts_empty_tool_calls() {
        let mut tail = msg("a1", "assistant", Some(false));
        tail.tool_calls = Some(vec![]);
        let messages = vec![msg("u1", "user", Some(false)), tail];
        assert!(has_completed_assistant_tail(&messages));
    }

    #[test]
    fn completed_assistant_tail_rejects_empty_transcript() {
        assert!(!has_completed_assistant_tail(&[]));
    }
}
