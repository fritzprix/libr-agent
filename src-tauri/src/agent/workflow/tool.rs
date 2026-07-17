use crate::agent::state::AgentSession;
use crate::agent::workflow::cancel::{
    discard_pending_events, should_consume_cancel_at_message_boundary,
};
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::session_repository::{SessionAttentionReason, SessionRepository};
use crate::repositories::SessionStatus;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Helper to handle tool result and trigger next steps if valid
pub async fn continue_workflow_after_tool(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: String,
    tool_call_id: String,
    result: crate::commands::agent_commands::ToolExecutionResult,
) -> Result<(), String> {
    use crate::mcp::types::MCPContent;

    match crate::agent::tools::handle_tool_result(
        active_sessions,
        app_handle,
        session_id.clone(),
        tool_call_id,
        result,
    )
    .await
    {
        Ok(Some((completed_message, all_completed))) => {
            let completed_messages = crate::agent::tools::spill_oversized_tool_result_messages(
                &session_id,
                vec![completed_message],
            )
            .await
            .map_err(|e| {
                log::error!(
                    "Failed to externalize oversized tool result messages for session {}: {}",
                    session_id,
                    e
                );
                e
            })?;

            // Ingest the completed tool message immediately!
            crate::services::MessageService::inject_messages_to_session(
                active_sessions,
                app_handle,
                &session_id,
                completed_messages.clone(),
                true,
            )
            .await
            .map_err(|e| {
                log::error!(
                    "Failed to inject tool result messages into session cache: {}",
                    e
                );
                e
            })?;

            if !all_completed {
                // If not all tools are completed, we are still waiting for other tools to execute.
                return Ok(());
            }

            log::info!(
                "All tool results received for session {}. Proceeding.",
                session_id
            );

            // [Race Mitigation] Verify session was not reset/cancelled before proceeding
            {
                let active = active_sessions.read().await;
                if let Some(session) = active.get(&session_id) {
                    if session.cancellation_token.is_cancelled() {
                        log::info!(
                            "Workflow was cancelled or reset for session {} before continuation. Discarding.",
                            session_id
                        );
                        return Ok(());
                    }
                }
            }

            // Message-boundary cancel handling:
            // If cancel was requested while tools were running, consume it now
            // after this message's full tool-call batch has completed.
            let should_stop_after_message = {
                let sessions = active_sessions.read().await;
                sessions
                    .get(&session_id)
                    .map(|session| session.cancel_pending.load(Ordering::SeqCst))
                    .unwrap_or(false)
            };

            if should_consume_cancel_at_message_boundary(should_stop_after_message) {
                log::info!(
                    "Consumed pending cancel at message boundary for session {}",
                    session_id
                );

                {
                    let mut sessions = active_sessions.write().await;
                    if let Some(session) = sessions.get_mut(&session_id) {
                        session.cancel_pending.store(false, Ordering::SeqCst);
                        session.cancellation_token = CancellationToken::new();
                    }
                }

                discard_pending_events(active_sessions, &session_id).await;

                let _ = crate::agent::lifecycle::update_session_status(
                    session_repo,
                    active_sessions,
                    app_handle,
                    &session_id,
                    SessionStatus::Paused,
                )
                .await;

                let event = crate::agent::events::AgentEvent::WorkflowCompleted {
                    session_id: session_id.clone(),
                    reason: crate::agent::events::WorkflowCompletionReason::Cancelled,
                };
                let _ = crate::agent::tauri_events::emit_agent_event(app_handle, event);
                return Ok(());
            }

            // Check for UI interaction (stop condition)
            let has_ui_interaction = {
                let sessions = active_sessions.read().await;
                if let Some(session) = sessions.get(&session_id) {
                    let msgs = session.messages.read().await;
                    let mut has_resource = false;
                    for msg in msgs.iter().rev() {
                        if msg.role == "assistant" {
                            break;
                        }
                        if msg.role == "tool" {
                            if msg
                                .content
                                .iter()
                                .any(|c| matches!(c, MCPContent::Resource { .. }))
                            {
                                has_resource = true;
                                break;
                            }
                        }
                    }
                    has_resource
                } else {
                    false
                }
            };

            if has_ui_interaction {
                log::info!(
                    "UI interaction detected for session {}. Stopping loop.",
                    session_id
                );
                match crate::agent::workflow::settle_session_and_go_idle(
                    session_repo,
                    active_sessions,
                    proxy_manager,
                    app_handle,
                    &session_id,
                    None,
                    crate::agent::events::WorkflowCompletionReason::RecurringStop,
                )
                .await
                {
                    Ok(true) => return Ok(()),
                    Ok(false) => {}
                    Err(error) => {
                        log::error!(
                            "Failed to settle session {} after UI interaction stop: {}",
                            session_id,
                            error
                        );
                        return Err(error);
                    }
                }
                let attention_at = chrono::Utc::now().timestamp_millis();
                if let Err(error) = session_repo
                    .update_attention(
                        &session_id,
                        attention_at,
                        SessionAttentionReason::RecurringStop,
                    )
                    .await
                {
                    log::error!(
                        "Failed to persist recurring-stop attention for session {}: {}",
                        session_id,
                        error
                    );
                }
            } else {
                // Check status before requesting LLM completion (Defense in depth against race condition)
                {
                    let sessions = active_sessions.read().await;
                    if let Some(session) = sessions.get(&session_id) {
                        if session.metadata.status != crate::repositories::SessionStatus::Busy {
                            log::info!(
                                "Skipping workflow restart for session {} (status: {:?})",
                                session_id,
                                session.metadata.status
                            );
                            return Ok(());
                        }
                    }
                }

                // Request next LLM completion
                if let Err(e) = crate::agent::llm::request_llm_completion_with_recovery(
                    session_repo,
                    active_sessions,
                    proxy_manager,
                    app_handle,
                    session_id,
                )
                .await
                {
                    log::error!("Failed to request LLM completion: {}", e);
                    return Err(format!("Failed to request LLM completion: {}", e));
                }
            }
        }
        Ok(Option::None) => {
            // Still waiting for other tools
        }
        Err(e) => {
            // Handle cancellation gracefully without emitting error event
            if e == "Workflow was cancelled" {
                log::info!(
                    "Ignoring tool result for session {} because the workflow was cancelled",
                    session_id
                );
                return Err(e);
            }

            log::error!("Error handling tool result: {}", e);
            if let Err(err) = crate::agent::llm::handle_llm_error(
                session_repo,
                active_sessions,
                app_handle,
                session_id,
                e.clone().into(),
            )
            .await
            {
                log::error!("Failed to handle LLM error: {}", err);
            }
            return Err(e);
        }
    }
    Ok(())
}
