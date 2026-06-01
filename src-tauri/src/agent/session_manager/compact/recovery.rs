use super::complete_compaction_with_hard_fallback;
use super::persistence::{spawn_resume_completion, CompactSummaryPersistenceContext};
use crate::agent::compact_recovery::handle_compact_error_state;
use crate::agent::events::AgentEventDispatcher;
use crate::agent::llm::types::{
    AgentRuntimeError, AgentRuntimeErrorType, CompactStateEvent, CompactStatePhase,
};
use crate::agent::state::{AgentSession, CompactionRecoveryPhase, CompactionSnapshot};
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

const MAX_COMPACTION_RETRY_ATTEMPTS: u32 = 3;

pub fn should_retry_budget_related_blocking_compaction(
    snapshot: &CompactionSnapshot,
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
        || normalized_message.contains("empty response from streamchat")
}

pub async fn handle_compact_error_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: String,
    error: AgentRuntimeError,
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
            compaction.clear_in_flight_state(false).await;
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

        if snapshot.blocks_workflow() {
            if let Some(compact_event) = compaction.current_request().await {
                return complete_compaction_with_hard_fallback(
                    CompactSummaryPersistenceContext {
                        active_sessions,
                        app_handle,
                        session_repo,
                        proxy_manager,
                        session_id: &session_id,
                        session_name: Some(session_name),
                        to_id: compact_event.to_id.clone(),
                        compacted_delta_count: compact_event.compacted_delta_count,
                    },
                    &error.display_message,
                    Some(&snapshot),
                )
                .await
                .map(|_| ());
            }
        }

        compaction.reset_recovery_progress().await;
    }

    handle_compact_error_state(session_repo, active_sessions, dispatcher, session_id, error).await
}
