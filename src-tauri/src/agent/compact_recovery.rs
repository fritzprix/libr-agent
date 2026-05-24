use crate::agent::events::{AgentEvent, AgentEventDispatcher};
use crate::agent::lifecycle::update_session_status_with_dispatcher;
use crate::agent::llm::types::{AgentRuntimeError, CompactStateEvent, CompactStatePhase};
use crate::agent::state::{AgentSession, CompactRepairState};
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

async fn clear_compaction_state(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    clear_last_compacted_tail_id: bool,
) {
    let compaction = {
        let active = active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| session.compaction.clone())
    };

    let Some(compaction) = compaction else {
        return;
    };
    compaction
        .clear_runtime_state(clear_last_compacted_tail_id)
        .await;
}

pub async fn clear_compact_in_flight(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) {
    clear_compaction_state(active_sessions, session_id, false).await;
}

pub async fn handle_compact_error_state(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: String,
    error: AgentRuntimeError,
) -> Result<(), String> {
    let (snapshot, session_name) = {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            (
                session.compaction.snapshot().await,
                session
                    .metadata
                    .name
                    .clone()
                    .unwrap_or_else(|| session_id.chars().take(8).collect::<String>()),
            )
        } else {
            (
                crate::agent::state::CompactionSnapshot {
                    phase: crate::agent::state::CompactionPhase::Idle,
                    last_compacted_tail_id: None,
                },
                session_id.chars().take(8).collect::<String>(),
            )
        }
    };
    let elapsed_ms = snapshot
        .started_at_ms()
        .map(|started_at| chrono::Utc::now().timestamp_millis() - started_at);

    let error_code = error
        .details
        .as_ref()
        .and_then(|details| details.error_code.as_deref())
        .unwrap_or("none");

    clear_compaction_state(active_sessions, &session_id, true).await;
    rearm_compact_repair_after_failure(active_sessions, &session_id).await;

    let state_event = CompactStateEvent {
        session_id: session_id.clone(),
        session_name: Some(session_name),
        compacting: false,
        awaiting_compact: false,
        phase: CompactStatePhase::Failed,
        error: Some(error.display_message.clone()),
    };

    dispatcher.emit_compact_state(state_event)?;

    log::warn!(
        "❌ Compaction failed: session={}, mode={}, elapsed_ms={}, error_code={}, message={}",
        session_id,
        snapshot.mode_label(),
        elapsed_ms
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        error_code,
        error.display_message
    );

    if snapshot.blocks_workflow() {
        log::warn!(
            "Blocking compaction failed for session {}. Failing workflow.",
            session_id
        );
        update_session_status_with_dispatcher(
            session_repo,
            active_sessions,
            dispatcher,
            &session_id,
            SessionStatus::Error,
        )
        .await?;

        dispatcher.emit_agent_event(AgentEvent::WorkflowError {
            session_id,
            error: error.clone(),
        })?;
    }

    Ok(())
}

async fn rearm_compact_repair_after_failure(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        if session.compact_repair_state() == CompactRepairState::Attempted {
            session.set_compact_repair_state(CompactRepairState::Needed);
        }
    }
}
