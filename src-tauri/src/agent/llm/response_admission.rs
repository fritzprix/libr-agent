use crate::agent::state::AgentSession;
use crate::repositories::SessionStatus;
use agent_response_guards::{
    inspect_response_admission as inspect_response_admission_guard,
    validate_expected_response_id as validate_expected_response_id_guard, GuardSessionStatus,
    ResponseAdmissionDecision, ORPHANED_UI_TOOL_RESULT_ERROR,
};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(crate) struct ResponseAdmission {
    pub should_mark_busy: bool,
    pub skip_expected_response_id_check: bool,
}

fn map_session_status(status: &SessionStatus) -> GuardSessionStatus {
    match status {
        SessionStatus::Idle => GuardSessionStatus::Idle,
        SessionStatus::Busy => GuardSessionStatus::Busy,
        SessionStatus::Paused => GuardSessionStatus::Paused,
        _ => GuardSessionStatus::Other,
    }
}

pub(crate) async fn inspect_response_admission(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    allow_idle_tool_entry: bool,
    is_ui_tool: bool,
    is_internal_ui_callback: bool,
) -> Result<ResponseAdmission, String> {
    let active = active_sessions.read().await;
    let Some(session) = active.get(session_id) else {
        return Ok(ResponseAdmission {
            should_mark_busy: false,
            skip_expected_response_id_check: false,
        });
    };

    let token_cancelled = session.cancellation_token.is_cancelled();
    let cancel_pending = session.cancel_pending.load(Ordering::SeqCst);
    let status = session.metadata.status.clone();

    let decision = inspect_response_admission_guard(
        map_session_status(&status),
        token_cancelled,
        cancel_pending,
        allow_idle_tool_entry,
        is_ui_tool,
        is_internal_ui_callback,
    );

    match decision {
        Ok(ResponseAdmissionDecision {
            should_mark_busy,
            skip_expected_response_id_check,
        }) => Ok(ResponseAdmission {
            should_mark_busy,
            skip_expected_response_id_check,
        }),
        Err(ORPHANED_UI_TOOL_RESULT_ERROR) => {
            if token_cancelled || cancel_pending {
                log::info!(
                    "Ignoring orphaned UI tool result for session {} (token_cancelled={}, cancel_pending={}, status={:?})",
                    session_id,
                    token_cancelled,
                    cancel_pending,
                    status
                );
            } else {
                log::info!(
                    "Ignoring orphaned UI tool result for session {} (status={:?}, has_tool_calls={})",
                    session_id,
                    status,
                    allow_idle_tool_entry
                );
            }
            Err(ORPHANED_UI_TOOL_RESULT_ERROR.to_string())
        }
        Err(error) if is_ui_tool => {
            log::info!(
                "Ignoring orphaned UI tool result for session {} (status={:?}, has_tool_calls={}, error={})",
                session_id,
                status,
                allow_idle_tool_entry,
                error
            );
            Err(ORPHANED_UI_TOOL_RESULT_ERROR.to_string())
        }
        Err(error) => {
            if token_cancelled || cancel_pending {
                log::info!(
                    "Workflow cancelled for session: {} (token_cancelled={}, cancel_pending={}, status={:?})",
                    session_id,
                    token_cancelled,
                    cancel_pending,
                    status
                );
            } else {
                log::info!(
                    "Rejecting LLM response for session {} (status={:?}, has_tool_calls={}, is_ui_tool={})",
                    session_id,
                    status,
                    allow_idle_tool_entry,
                    is_ui_tool
                );
            }
            Err(error.to_string())
        }
    }
}

pub(crate) async fn clear_cancel_pending_flag(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) {
    let active = active_sessions.read().await;
    if let Some(session) = active.get(session_id) {
        session.cancel_pending.store(false, Ordering::SeqCst);
    }
}

pub(crate) async fn consume_expected_response_id(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    received_message_id: &str,
    is_ui_tool: bool,
) -> Result<(), String> {
    let sessions = active_sessions.read().await;
    let Some(session) = sessions.get(session_id) else {
        return Ok(());
    };

    let mut expected_id = session.expected_response_id.write().await;
    validate_expected_response_id(
        session_id,
        expected_id.as_deref(),
        received_message_id,
        is_ui_tool,
    )?;
    expected_id.take();
    Ok(())
}

pub(crate) fn validate_expected_response_id(
    session_id: &str,
    expected_response_id: Option<&str>,
    received_message_id: &str,
    is_ui_tool: bool,
) -> Result<(), String> {
    match validate_expected_response_id_guard(expected_response_id, received_message_id, is_ui_tool)
    {
        Ok(()) => Ok(()),
        Err(ORPHANED_UI_TOOL_RESULT_ERROR) => {
            if let Some(expected_response_id) = expected_response_id {
                log::info!(
                    "Ignoring orphaned UI tool result for session {} (expected_response_id={}, received_message_id={})",
                    session_id,
                    expected_response_id,
                    received_message_id
                );
            } else {
                log::info!(
                    "Ignoring orphaned UI tool result for session {} (received_message_id={}, expected_response_id=<none>)",
                    session_id,
                    received_message_id
                );
            }
            Err(ORPHANED_UI_TOOL_RESULT_ERROR.to_string())
        }
        Err(error) => {
            if let Some(expected_response_id) = expected_response_id {
                log::info!(
                    "Ignoring superseded LLM response for session {} (expected_response_id={}, received_message_id={})",
                    session_id,
                    expected_response_id,
                    received_message_id
                );
            } else {
                log::info!(
                    "Ignoring stray LLM response for session {} (received_message_id={}, expected_response_id=<none>)",
                    session_id,
                    received_message_id
                );
            }
            Err(error.to_string())
        }
    }
}
