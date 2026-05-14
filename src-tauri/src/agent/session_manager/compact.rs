use super::AgentSessionManager;
use crate::agent::compact_recovery::{clear_compact_in_flight, handle_compact_error_state};
use crate::agent::events::AgentEventDispatcher;
use crate::agent::llm::types::CompactStatePhase;
use crate::agent::state::{AgentSession, DeferredWorkflowStep};
use crate::agent::tauri_events::emit_compact_finished;
use crate::repositories::{CompactContextRecord, SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

fn spawn_resume_completion(
    manager: &AgentSessionManager,
    session_id: &str,
    log_label: &'static str,
) {
    let session_repo = manager.session_repo.clone();
    let active_sessions = manager.active_sessions.clone();
    let proxy_manager = manager.proxy_manager.clone();
    let app_handle = manager.app_handle.clone();
    let resume_session_id = session_id.to_string();

    tokio::spawn(async move {
        if let Err(error) = crate::agent::llm::request_llm_completion_with_recovery(
            &session_repo,
            &active_sessions,
            &proxy_manager,
            &app_handle,
            resume_session_id.clone(),
        )
        .await
        {
            log::error!(
                "Failed to resume {} after compaction for session {}: {}",
                log_label,
                resume_session_id,
                error
            );
        }
    });
}

fn spawn_resume_tool_execution(
    manager: &AgentSessionManager,
    session_id: &str,
    tool_calls: Vec<crate::agent::types::ToolCall>,
) {
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

async fn finalize_workflow_completion(
    manager: &AgentSessionManager,
    session_id: &str,
    reason: crate::agent::events::WorkflowCompletionReason,
) -> Result<(), String> {
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
        .map_err(|e| format!("Failed to emit event: {}", e))
}

pub async fn handle_compact_error_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: String,
    error: crate::agent::llm::types::AgentRuntimeError,
) -> Result<(), String> {
    handle_compact_error_state(session_repo, active_sessions, dispatcher, session_id, error).await
}

pub async fn handle_compact_response(
    manager: &AgentSessionManager,
    session_id: &str,
    from_id: String,
    to_id: String,
    summary: String,
) -> Result<(), String> {
    let (compaction, session_name) = {
        let active = manager.active_sessions.read().await;
        active.get(session_id).map_or((None, None), |session| {
            (
                Some(session.compaction.clone()),
                session.metadata.name.clone(),
            )
        })
    };
    let started_at_ms = if let Some(compaction) = compaction {
        compaction.snapshot().await.started_at_ms
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

    let completion_plan = {
        let active = manager.active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            Some(session.compaction.take_completion_plan().await)
        } else {
            None
        }
    };
    let should_resume_completion = completion_plan
        .as_ref()
        .map(|plan| plan.should_resume_completion)
        .unwrap_or(false);
    let deferred_workflow_step = completion_plan.and_then(|plan| plan.deferred_workflow_step);

    log::info!(
        "📌 Compact completion decision for session {}: should_resume_completion={}, deferred_step={}",
        session_id,
        should_resume_completion,
        deferred_workflow_step.is_some()
    );

    clear_compact_in_flight(&manager.active_sessions, session_id).await;
    if let Err(error) = emit_compact_finished(
        &manager.app_handle,
        session_id.to_string(),
        session_name,
        CompactStatePhase::Succeeded,
        None,
    ) {
        log::warn!(
            "Failed to emit compact finished state for session {} after successful compaction: {}",
            session_id,
            error
        );
    }

    if let Some(deferred_workflow_step) = deferred_workflow_step {
        match deferred_workflow_step {
            DeferredWorkflowStep::RequestCompletion => {
                log::info!(
                    "▶️ Resuming deferred LLM completion after compaction for session {}",
                    session_id
                );
                spawn_resume_completion(manager, session_id, "deferred LLM completion");
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
                spawn_resume_tool_execution(manager, session_id, tool_calls);
            }
            DeferredWorkflowStep::FinalizeWorkflow { reason } => {
                finalize_workflow_completion(manager, session_id, reason).await?;
            }
        }
    } else if should_resume_completion {
        log::info!(
            "▶️ Resuming blocked LLM completion after compaction for session {}",
            session_id
        );
        spawn_resume_completion(manager, session_id, "LLM completion");
    }

    Ok(())
}
