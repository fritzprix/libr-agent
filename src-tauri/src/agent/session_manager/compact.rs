use crate::agent::compact_recovery::handle_compact_error_state;
use crate::agent::events::AgentEventDispatcher;
use crate::agent::llm::types::{
    AgentRuntimeError, AgentRuntimeErrorType, CompactStateEvent, CompactStatePhase,
};
use crate::agent::state::{AgentSession, CompactionRecoveryPhase, CompactionResumeAction};
use crate::agent::tauri_events::emit_compact_finished;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::compact_context_repository::CompactContextRepository;
use crate::repositories::{CompactContextRecord, SessionRepository};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

const MAX_COMPACTION_RETRY_ATTEMPTS: u32 = 3;

pub fn should_retry_budget_related_blocking_compaction(
    snapshot: &crate::agent::state::CompactionSnapshot,
    error: &AgentRuntimeError,
) -> bool {
    if !snapshot.blocks_workflow()
        || matches!(
            snapshot.recovery_phase,
            CompactionRecoveryPhase::DegradedTools
        )
    {
        return false;
    }

    if matches!(error.error_type, AgentRuntimeErrorType::ContextLimitError) {
        return true;
    }

    let error_code = error
        .details
        .as_ref()
        .and_then(|details| details.error_code.as_deref());
    if matches!(
        error_code,
        Some("CONTEXT_LIMIT_EXCEEDED") | Some("RUST_PREFLIGHT_CONTEXT_LIMIT")
    ) {
        return true;
    }

    let normalized_message = error.display_message.to_lowercase();
    normalized_message.contains("prompt too long")
        || normalized_message.contains("exceeds max context window")
        || normalized_message.contains("maximum context length")
}

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

pub async fn handle_compact_error_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: String,
    error: crate::agent::llm::types::AgentRuntimeError,
) -> Result<(), String> {
    let (compaction, snapshot, session_name) = {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            (
                Some(session.compaction.clone()),
                Some(session.compaction.snapshot().await),
                session
                    .metadata
                    .name
                    .clone()
                    .unwrap_or_else(|| session_id.chars().take(8).collect::<String>()),
            )
        } else {
            (None, None, session_id.chars().take(8).collect::<String>())
        }
    };

    if let (Some(compaction), Some(snapshot)) = (compaction, snapshot) {
        if should_retry_budget_related_blocking_compaction(&snapshot, &error) {
            let transition_label = match snapshot.recovery_phase {
                CompactionRecoveryPhase::CacheAligned
                    if snapshot.retry_attempt < MAX_COMPACTION_RETRY_ATTEMPTS =>
                {
                    let retry_attempt = compaction.increment_retry_attempt().await;
                    format!("budget-retry-{}", retry_attempt)
                }
                CompactionRecoveryPhase::CacheAligned => {
                    compaction.transition_to_overflow_recovery().await;
                    "overflow-recovery".to_string()
                }
                CompactionRecoveryPhase::OverflowRecovery => {
                    compaction.transition_to_degraded_tools().await;
                    "degraded-tools".to_string()
                }
                CompactionRecoveryPhase::DegradedTools => unreachable!(),
            };
            compaction.clear_runtime_state(false).await;
            dispatcher.emit_compact_state(CompactStateEvent {
                session_id: session_id.clone(),
                session_name: Some(session_name),
                compacting: false,
                awaiting_compact: false,
                phase: CompactStatePhase::Failed,
                error: Some(error.display_message.clone()),
            })?;
            log::warn!(
                "🔁 Advancing compaction overflow recovery after budget-related failure: session={}, next_step={}",
                session_id,
                transition_label
            );
            spawn_resume_completion(
                session_repo,
                active_sessions,
                proxy_manager,
                app_handle,
                &session_id,
                "LLM completion after compaction overflow",
            );
            return Ok(());
        }

        compaction.reset_recovery_progress().await;
    }

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

pub async fn handle_compact_response(params: CompactResponseParams<'_>) -> Result<(), String> {
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
        CompactionResumeAction::ResumeCompletion => {
            log::info!(
                "▶️ Resuming blocked LLM completion after compaction for session {}",
                session_id
            );
            spawn_resume_completion(
                session_repo,
                active_sessions,
                proxy_manager,
                app_handle,
                session_id,
                "LLM completion",
            );
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
