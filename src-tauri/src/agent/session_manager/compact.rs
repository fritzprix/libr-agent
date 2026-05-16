use crate::agent::compact_recovery::handle_compact_error_state;
use crate::agent::events::AgentEventDispatcher;
use crate::agent::llm::types::CompactStatePhase;
use crate::agent::state::{AgentSession, CompactionResumeAction, DeferredWorkflowStep};
use crate::agent::tauri_events::emit_compact_finished;
use crate::repositories::{CompactContextRecord, SessionRepository, SessionStatus};
use crate::repositories::compact_context_repository::CompactContextRepository;
use crate::mcp::MCPServiceProxyManager;
use tauri::AppHandle;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

fn spawn_resume_completion(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: &str,
    log_label: &'static str,
) {
    let session_repo = session_repo.clone();
    let active_sessions = active_sessions.clone();
    let proxy_manager = proxy_manager.clone();
    let app_handle = app_handle.clone();
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
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: &str,
    tool_calls: Vec<crate::agent::types::ToolCall>,
) {
    let session_repo = session_repo.clone();
    let active_sessions = active_sessions.clone();
    let proxy_manager = proxy_manager.clone();
    let app_handle = app_handle.clone();
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
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    reason: crate::agent::events::WorkflowCompletionReason,
) -> Result<(), String> {
    crate::agent::lifecycle::update_session_status(
        session_repo,
        active_sessions,
        app_handle,
        session_id,
        SessionStatus::Idle,
    )
    .await?;

    let event = crate::agent::events::AgentEvent::WorkflowCompleted {
        session_id: session_id.to_string(),
        reason,
    };
    crate::agent::tauri_events::emit_agent_event(app_handle, event)
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

pub struct CompactResponseParams<'a> {
    pub active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
    pub app_handle: &'a AppHandle,
    pub session_repo: &'a Arc<dyn SessionRepository>,
    pub proxy_manager: &'a Arc<MCPServiceProxyManager>,
    pub session_id: &'a str,
    pub from_id: String,
    pub to_id: String,
    pub summary: String,
}

pub async fn handle_compact_response(
    params: CompactResponseParams<'_>,
) -> Result<(), String> {
    let CompactResponseParams {
        active_sessions,
        app_handle,
        session_repo,
        proxy_manager,
        session_id,
        from_id,
        to_id,
        summary,
    } = params;
    let (compaction, session_name) = {
        let active = active_sessions.read().await;
        active.get(session_id).map_or((None, None), |session| {
            (
                Some(session.compaction.clone()),
                session.metadata.name.clone(),
            )
        })
    };
    let started_at_ms = if let Some(compaction) = compaction {
        compaction.snapshot().await.started_at_ms()
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
    save_compact_context(active_sessions, session_id, record).await?;

    let resume_action = {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            Some(session.compaction.complete_success().await)
        } else {
            None
        }
    };
    let resume_action = resume_action.unwrap_or(CompactionResumeAction::Nothing);

    log::info!(
        "📌 Compact completion decision for session {}: action={}",
        session_id,
        match &resume_action {
            CompactionResumeAction::Nothing => "nothing",
            CompactionResumeAction::ResumeCompletion => "resume_completion",
            CompactionResumeAction::RunDeferred(_) => "run_deferred",
        }
    );
    if let Err(error) = emit_compact_finished(
        app_handle,
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

    match resume_action {
        CompactionResumeAction::RunDeferred(deferred_workflow_step) => match deferred_workflow_step
        {
            DeferredWorkflowStep::RequestCompletion => {
                log::info!(
                    "▶️ Resuming deferred LLM completion after compaction for session {}",
                    session_id
                );
                spawn_resume_completion(session_repo, active_sessions, proxy_manager, app_handle, session_id, "deferred LLM completion");
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
                    let mut active = active_sessions.write().await;
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
                spawn_resume_tool_execution(session_repo, active_sessions, proxy_manager, app_handle, session_id, tool_calls);
            }
            DeferredWorkflowStep::FinalizeWorkflow { reason } => {
                finalize_workflow_completion(session_repo, active_sessions, app_handle, session_id, reason).await?;
            }
        },
        CompactionResumeAction::ResumeCompletion => {
            log::info!(
                "▶️ Resuming blocked LLM completion after compaction for session {}",
                session_id
            );
            spawn_resume_completion(session_repo, active_sessions, proxy_manager, app_handle, session_id, "LLM completion");
        }
        CompactionResumeAction::Nothing => {}
    }

    Ok(())
}

pub async fn get_compact_context(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<Option<CompactContextRecord>, String> {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        let compact = session.compact_context.read().await;
        if compact.is_some() {
            return Ok((*compact).clone());
        }
    }

    let repo = crate::state::get_compact_context_repository();
    repo.get_by_session_id(session_id)
        .await
        .map_err(|e| e.to_string())
}

pub async fn save_compact_context(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    record: CompactContextRecord,
) -> Result<(), String> {
    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(session_id) {
            let mut compact = session.compact_context.write().await;
            *compact = Some(record.clone());
        }
    }

    let repo = crate::state::get_compact_context_repository();
    repo.upsert(&record).await.map_err(|e| e.to_string())?;

    Ok(())
}

pub async fn wait_for_compaction_to_settle(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let started_at = std::time::Instant::now();

    loop {
        let is_settled = {
            let active = active_sessions.read().await;
            let session = active
                .get(session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;
            session.compaction.is_settled()
        };

        if is_settled {
            return Ok(());
        }

        if started_at.elapsed() >= timeout {
            return Err(format!(
                "Timed out waiting for compaction to settle for session {} after {} seconds",
                session_id,
                timeout.as_secs()
            ));
        }

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}
