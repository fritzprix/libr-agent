use crate::agent::events::{AgentEvent, AgentEventDispatcher};
use crate::agent::lifecycle::update_session_status_with_dispatcher;
use crate::agent::llm::types::{AgentRuntimeError, CompactStateEvent, CompactStatePhase};
use crate::agent::state::AgentSession;
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactErrorAction {
    None,
    FinalizeWorkflow,
}

async fn clear_compaction_state(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    clear_last_compacted_tail_id: bool,
) {
    let handles = {
        let active = active_sessions.read().await;
        active.get(session_id).map(|session| {
            session.compact_in_flight.store(false, Ordering::SeqCst);
            session
                .awaiting_compact_completion
                .store(false, Ordering::SeqCst);
            session
                .finalize_workflow_after_compact
                .store(false, Ordering::SeqCst);
            (
                session.compact_started_at_ms.clone(),
                clear_last_compacted_tail_id.then(|| session.last_compacted_tail_id.clone()),
                session.deferred_workflow_step.clone(),
            )
        })
    };

    let Some((
        compact_started_at_ms_handle,
        last_compacted_tail_id_handle,
        deferred_workflow_step_handle,
    )) = handles
    else {
        return;
    };

    if let Some(last_compacted_tail_id_handle) = last_compacted_tail_id_handle {
        *last_compacted_tail_id_handle.write().await = None;
    }

    *deferred_workflow_step_handle.write().await = None;
    *compact_started_at_ms_handle.write().await = None;
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
) -> Result<CompactErrorAction, String> {
    let (
        was_awaiting,
        should_finalize_after_compact,
        deferred_workflow_step,
        session_name,
        compact_started_at_ms_handle,
    ) = {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            (
                session.awaiting_compact_completion.load(Ordering::SeqCst),
                session
                    .finalize_workflow_after_compact
                    .load(Ordering::SeqCst),
                session.deferred_workflow_step.read().await.clone(),
                session
                    .metadata
                    .name
                    .clone()
                    .unwrap_or_else(|| session_id.chars().take(8).collect::<String>()),
                Some(session.compact_started_at_ms.clone()),
            )
        } else {
            (
                false,
                false,
                None,
                session_id.chars().take(8).collect::<String>(),
                None,
            )
        }
    };
    let elapsed_ms = if let Some(compact_started_at_ms_handle) = compact_started_at_ms_handle {
        compact_started_at_ms_handle
            .read()
            .await
            .map(|started_at| chrono::Utc::now().timestamp_millis() - started_at)
    } else {
        None
    };

    let error_code = error
        .details
        .as_ref()
        .and_then(|details| details.error_code.as_deref())
        .unwrap_or("none");

    clear_compaction_state(active_sessions, &session_id, true).await;

    let state_event = CompactStateEvent {
        session_id: session_id.clone(),
        session_name: Some(session_name),
        compacting: false,
        phase: CompactStatePhase::Failed,
        error: Some(error.display_message.clone()),
    };

    dispatcher.emit_compact_state(state_event)?;

    log::warn!(
        "❌ Compaction failed: session={}, mode={}, elapsed_ms={}, error_code={}, message={}",
        session_id,
        if was_awaiting {
            "preflight"
        } else {
            "background"
        },
        elapsed_ms
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        error_code,
        error.display_message
    );

    if was_awaiting || deferred_workflow_step.is_some() {
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

        Ok(CompactErrorAction::None)
    } else if should_finalize_after_compact {
        Ok(CompactErrorAction::FinalizeWorkflow)
    } else {
        Ok(CompactErrorAction::None)
    }
}
