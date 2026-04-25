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
        Ok(Some(accumulated_messages)) => {
            log::info!(
                "All tool results received for session {}. Proceeding.",
                session_id
            );

            let accumulated_messages = crate::agent::tools::spill_oversized_tool_result_messages(
                &session_id,
                accumulated_messages,
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

            // Use MessageService to handle message caching, event emission, and DB persistence.
            // Propagate errors so the LLM loop does not continue with a stale context window
            // if injection fails (e.g. due to a DB initialization error).
            crate::services::MessageService::inject_messages_to_session(
                active_sessions,
                app_handle,
                &session_id,
                accumulated_messages.clone(),
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
            let has_ui_interaction = accumulated_messages.iter().any(|msg| {
                msg.content
                    .iter()
                    .any(|c| matches!(c, MCPContent::Resource { .. }))
            });

            if has_ui_interaction {
                log::info!(
                    "UI interaction detected for session {}. Stopping loop.",
                    session_id
                );
                if let Err(error) = crate::agent::lifecycle::update_session_status(
                    session_repo,
                    active_sessions,
                    app_handle,
                    &session_id,
                    SessionStatus::Idle,
                )
                .await
                {
                    log::error!(
                        "Failed to persist idle status after UI interaction stop for session {}: {}",
                        session_id,
                        error
                    );
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
                let event = crate::agent::events::AgentEvent::WorkflowCompleted {
                    session_id: session_id.clone(),
                    reason: crate::agent::events::WorkflowCompletionReason::RecurringStop,
                };
                if let Err(error) = crate::agent::tauri_events::emit_agent_event(app_handle, event)
                {
                    log::error!(
                        "Failed to emit recurring-stop completion event for session {}: {}",
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
