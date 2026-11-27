//! Terminal assistant persistence and status transition primitives.

use crate::agent::events::{AgentEvent, AgentEventDispatcher};
use crate::agent::llm::types::{AgentRuntimeError, AgentRuntimeErrorType};
use crate::agent::state::AgentSession;
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::{SessionRepository, SessionStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::messages::{
    emit_settlement_agent_event, user_facing_persist_session_missing_error,
    user_facing_persist_storage_error,
};
use super::pending::continue_workflow_if_pending_events;
use super::WorkflowRestart;
use super::{
    finalize_session_error_with_dispatcher, finalize_session_error_with_hook_sink,
    spawn_stop_failure,
};

pub(super) fn latest_assistant_message_from_cache(messages: &[Message]) -> Option<Message> {
    messages
        .iter()
        .rev()
        .find(|message| message.role == "assistant")
        .cloned()
}

/// Persist the terminal assistant message synchronously before exposing `Idle`.
///
/// When `terminal_message` is provided it is written directly; otherwise the latest
/// assistant row from the in-memory cache is used.
pub async fn persist_terminal_assistant_sync(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    terminal_message: Option<&Message>,
) -> Result<(), String> {
    let message_to_persist = if let Some(message) = terminal_message {
        Some(message.clone())
    } else {
        let sessions = active_sessions.read().await;
        let session = sessions.get(session_id).ok_or_else(|| {
            log::error!(
                "Terminal persist cache lookup failed: session {} is not in active_sessions",
                session_id
            );
            user_facing_persist_session_missing_error()
        })?;
        let messages = session.messages.read().await;
        latest_assistant_message_from_cache(&messages)
    };

    let Some(message) = message_to_persist else {
        return Ok(());
    };

    let repo = crate::state::get_message_repository();
    repo.insert(&message).await.map_err(|error| {
        log::error!(
            "Failed to persist terminal assistant message: session={}, msg_id={}, error={}",
            session_id,
            message.id,
            error
        );
        user_facing_persist_storage_error()
    })
}

/// Persist terminal messages and run the first pending-work scan.
///
/// Returns `Ok(true)` when pending events restarted the workflow. The success
/// path still performs a second pending scan in
/// [`settle_session_and_go_idle_with_dispatcher`] to close the finish-window
/// race that can occur after this function returns.
pub async fn settle_before_terminal_transition(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    restart: &dyn WorkflowRestart,
    session_id: &str,
    terminal_message: Option<&Message>,
) -> Result<bool, String> {
    persist_terminal_assistant_sync(active_sessions, session_id, terminal_message).await?;

    continue_workflow_if_pending_events(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        restart,
        session_id,
    )
    .await
}

pub(super) async fn mark_error_after_persist_failure(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    user_facing_error: &str,
) -> Result<(), String> {
    log::error!(
        "Terminal persist failed for session {}. Transitioning to Error. user_message={}",
        session_id,
        user_facing_error
    );
    let workflow_error =
        AgentRuntimeError::new(AgentRuntimeErrorType::AiServiceError, user_facing_error)
            .with_code("TERMINAL_PERSIST_FAILED");

    if let Err(mark_error) = finalize_session_error_with_dispatcher(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        workflow_error,
    )
    .await
    {
        log::error!(
            "Session {} could not complete Error settlement after terminal persist failure: {}. \
             StatusChanged may still have reached the UI.",
            session_id,
            mark_error
        );
    }

    Ok(())
}

pub(super) async fn handle_terminal_persist_failure(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    persist_error: String,
) -> Result<(), String> {
    if let Err(mark_error) = mark_error_after_persist_failure(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        &persist_error,
    )
    .await
    {
        log::error!(
            "Persist failed for session {} and marking Error also failed: {}",
            session_id,
            mark_error
        );
    }
    Err(persist_error)
}

/// Terminal status plus the UI event and its log label for one settlement.
///
/// The three values only ever travel together, so grouping them keeps both
/// settlement entry points under the clippy argument limit without a lint
/// allow (#3071 G1.1).
pub(super) struct TerminalTransition {
    pub status: SessionStatus,
    pub event: AgentEvent,
    pub event_label: &'static str,
}

pub(super) async fn transition_to_terminal_status_and_emit(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    transition: TerminalTransition,
) -> Result<(), String> {
    transition_to_terminal_status_and_emit_with_hook_sink(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        transition,
        spawn_stop_failure,
    )
    .await
}

pub(super) async fn transition_to_terminal_status_and_emit_with_hook_sink(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: &str,
    transition: TerminalTransition,
    emit_stop_failure: impl FnOnce(Arc<RwLock<HashMap<String, AgentSession>>>, String, &'static str),
) -> Result<(), String> {
    let TerminalTransition {
        status,
        event,
        event_label,
    } = transition;

    // Error settlements share one helper so every Error path emits
    // `WorkflowError` before fire-and-forget `StopFailure` (#3071 G1.1/G1.2).
    if status == SessionStatus::Error {
        let error = match event {
            AgentEvent::WorkflowError { error, .. } => error,
            other => {
                return Err(format!(
                    "Error settlement requires WorkflowError, got {other:?}"
                ));
            }
        };
        return finalize_session_error_with_hook_sink(
            session_repo,
            active_sessions,
            dispatcher,
            session_id,
            error,
            emit_stop_failure,
        )
        .await;
    }

    crate::agent::lifecycle::update_session_status_with_dispatcher(
        session_repo,
        active_sessions,
        dispatcher,
        session_id,
        status,
    )
    .await?;

    emit_settlement_agent_event(dispatcher, session_id, event, event_label)
}
