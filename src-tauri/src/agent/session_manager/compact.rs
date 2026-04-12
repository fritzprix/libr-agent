use super::AgentSessionManager;
use crate::agent::compact_recovery::{
    clear_compact_in_flight, handle_compact_error_state, CompactErrorAction,
};
use crate::agent::events::AgentEventDispatcher;
use crate::agent::state::{AgentSession, DeferredWorkflowStep};
use crate::repositories::{CompactContextRecord, SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn handle_compact_error_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: String,
    error: crate::agent::llm::types::AgentRuntimeError,
) -> Result<(), String> {
    if matches!(
        handle_compact_error_state(
            session_repo,
            active_sessions,
            dispatcher,
            session_id.clone(),
            error
        )
        .await?,
        CompactErrorAction::FinalizeWorkflow
    ) {
        if let Some(app_handle) = crate::state::get_app_handle() {
            crate::agent::lifecycle::update_session_status(
                session_repo,
                active_sessions,
                app_handle,
                &session_id,
                SessionStatus::Idle,
            )
            .await?;
        }

        dispatcher.emit_agent_event(crate::agent::events::AgentEvent::WorkflowCompleted {
            session_id,
            reason: crate::agent::events::WorkflowCompletionReason::Natural,
        })?;
    }

    Ok(())
}

pub async fn handle_compact_response(
    manager: &AgentSessionManager,
    session_id: &str,
    from_id: String,
    to_id: String,
    summary: String,
) -> Result<(), String> {
    let compact_started_at_ms_handle = {
        let active = manager.active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| session.compact_started_at_ms.clone())
    };
    let started_at_ms = if let Some(compact_started_at_ms_handle) = compact_started_at_ms_handle {
        *compact_started_at_ms_handle.read().await
    } else {
        None
    };

    log::info!(
        "✅ Compact response stored for session {}: from_id={}, to_id={}, summary_chars={}, summary_est_tokens=~{}, elapsed_ms={}",
        session_id,
        from_id,
        to_id,
        summary.len(),
        summary.len() / 4,
        started_at_ms
            .map(|value| (chrono::Utc::now().timestamp_millis() - value).to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );

    let record = CompactContextRecord {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        from_id,
        to_id,
        summary,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    manager.save_compact_context(session_id, record).await?;

    let (should_resume_completion, should_finalize_after_compact, deferred_workflow_step) = {
        let active = manager.active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            (
                session
                    .awaiting_compact_completion
                    .swap(false, Ordering::SeqCst),
                session
                    .finalize_workflow_after_compact
                    .swap(false, Ordering::SeqCst),
                session.deferred_workflow_step.write().await.take(),
            )
        } else {
            (false, false, None)
        }
    };

    log::info!(
        "📌 Compact completion decision for session {}: should_resume_completion={}, should_finalize_after_compact={}, deferred_step={}",
        session_id,
        should_resume_completion,
        should_finalize_after_compact,
        deferred_workflow_step.is_some()
    );

    clear_compact_in_flight(&manager.active_sessions, session_id).await;

    if let Some(deferred_workflow_step) = deferred_workflow_step {
        match deferred_workflow_step {
            DeferredWorkflowStep::RequestCompletion => {
                log::info!(
                    "▶️ Resuming deferred LLM completion after compaction for session {}",
                    session_id
                );
                let session_repo = manager.session_repo.clone();
                let active_sessions = manager.active_sessions.clone();
                let proxy_manager = manager.proxy_manager.clone();
                let app_handle = manager.app_handle.clone();
                let resume_session_id = session_id.to_string();

                tokio::spawn(async move {
                    if let Err(error) = crate::agent::llm::request_llm_completion(
                        &session_repo,
                        &active_sessions,
                        &proxy_manager,
                        &app_handle,
                        resume_session_id.clone(),
                    )
                    .await
                    {
                        log::error!(
                            "Failed to resume deferred LLM completion after compaction for session {}: {}",
                            resume_session_id,
                            error
                        );

                        if let Err(handle_error) = crate::agent::llm::handle_llm_error(
                            &session_repo,
                            &active_sessions,
                            &app_handle,
                            resume_session_id.clone(),
                            error,
                        )
                        .await
                        {
                            log::error!(
                                "Failed to surface deferred post-compaction resume error for session {}: {}",
                                resume_session_id,
                                handle_error
                            );
                        }
                    }
                });
            }
            DeferredWorkflowStep::ExecuteToolCalls {
                assistant_message_id,
                tool_calls,
            } => {
                log::info!(
                    "▶️ Resuming deferred tool execution after compaction for session {} (assistant_message={}, tool_calls={})",
                    session_id,
                    assistant_message_id,
                    tool_calls.len()
                );

                {
                    let mut active = manager.active_sessions.write().await;
                    if let Some(session) = active.get_mut(session_id) {
                        let expected_tool_call_ids: std::collections::HashSet<String> =
                            tool_calls.iter().map(|tc| tc.id.clone()).collect();
                        session.pending_execution =
                            Some(crate::agent::state::PendingToolExecution {
                                message_id: assistant_message_id.clone(),
                                total_expected: tool_calls.len(),
                                results: Vec::new(),
                                tool_names: tool_calls
                                    .iter()
                                    .map(|tc| (tc.id.clone(), tc.function.name.clone()))
                                    .collect(),
                                expected_tool_call_ids,
                                completed_tool_call_ids: std::collections::HashSet::new(),
                            });
                    }
                }

                let session_repo = manager.session_repo.clone();
                let active_sessions = manager.active_sessions.clone();
                let proxy_manager = manager.proxy_manager.clone();
                let app_handle = manager.app_handle.clone();
                let resume_session_id = session_id.to_string();

                tokio::spawn(async move {
                    crate::agent::llm::tool_execution::execute_tool_calls(
                        session_repo,
                        active_sessions,
                        proxy_manager,
                        app_handle,
                        resume_session_id,
                        tool_calls,
                    )
                    .await;
                });
            }
            DeferredWorkflowStep::FinalizeWorkflow { reason } => {
                crate::agent::lifecycle::update_session_status(
                    &manager.session_repo,
                    &manager.active_sessions,
                    &manager.app_handle,
                    session_id,
                    SessionStatus::Idle,
                )
                .await?;

                let event = crate::agent::events::AgentEvent::WorkflowCompleted {
                    session_id: session_id.to_string(),
                    reason,
                };
                crate::agent::tauri_events::emit_agent_event(&manager.app_handle, event)
                    .map_err(|e| format!("Failed to emit event: {}", e))?;
            }
        }
    } else if should_resume_completion {
        log::info!(
            "▶️ Resuming blocked LLM completion after compaction for session {}",
            session_id
        );
        let session_repo = manager.session_repo.clone();
        let active_sessions = manager.active_sessions.clone();
        let proxy_manager = manager.proxy_manager.clone();
        let app_handle = manager.app_handle.clone();
        let resume_session_id = session_id.to_string();

        tokio::spawn(async move {
            if let Err(error) = crate::agent::llm::request_llm_completion(
                &session_repo,
                &active_sessions,
                &proxy_manager,
                &app_handle,
                resume_session_id.clone(),
            )
            .await
            {
                log::error!(
                    "Failed to resume LLM completion after compaction for session {}: {}",
                    resume_session_id,
                    error
                );

                if let Err(handle_error) = crate::agent::llm::handle_llm_error(
                    &session_repo,
                    &active_sessions,
                    &app_handle,
                    resume_session_id.clone(),
                    error,
                )
                .await
                {
                    log::error!(
                        "Failed to surface post-compaction resume error for session {}: {}",
                        resume_session_id,
                        handle_error
                    );
                }
            }
        });
    } else if should_finalize_after_compact {
        crate::agent::lifecycle::update_session_status(
            &manager.session_repo,
            &manager.active_sessions,
            &manager.app_handle,
            session_id,
            SessionStatus::Idle,
        )
        .await?;

        let event = crate::agent::events::AgentEvent::WorkflowCompleted {
            session_id: session_id.to_string(),
            reason: crate::agent::events::WorkflowCompletionReason::Natural,
        };
        crate::agent::tauri_events::emit_agent_event(&manager.app_handle, event)
            .map_err(|e| format!("Failed to emit event: {}", e))?;
    }

    Ok(())
}
