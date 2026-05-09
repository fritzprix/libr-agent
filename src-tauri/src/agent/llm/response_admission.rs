use crate::agent::state::AgentSession;
use crate::repositories::SessionStatus;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(crate) struct ResponseAdmission {
    pub should_mark_busy: bool,
}

pub(crate) async fn inspect_response_admission(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    allow_idle_tool_entry: bool,
) -> Result<ResponseAdmission, String> {
    let active = active_sessions.read().await;
    let Some(session) = active.get(session_id) else {
        return Ok(ResponseAdmission {
            should_mark_busy: false,
        });
    };

    let token_cancelled = session.cancellation_token.is_cancelled();
    let cancel_pending = session.cancel_pending.load(Ordering::SeqCst);
    let status = session.metadata.status.clone();

    if token_cancelled || cancel_pending {
        log::info!(
            "Workflow cancelled for session: {} (token_cancelled={}, cancel_pending={}, status={:?})",
            session_id,
            token_cancelled,
            cancel_pending,
            status
        );
        return Err("Workflow was cancelled".to_string());
    }

    if status == SessionStatus::Busy {
        return Ok(ResponseAdmission {
            should_mark_busy: false,
        });
    }

    if status == SessionStatus::Idle && allow_idle_tool_entry {
        return Ok(ResponseAdmission {
            should_mark_busy: true,
        });
    }

    log::info!(
        "Rejecting LLM response for session {} (status={:?}, has_tool_calls={})",
        session_id,
        status,
        allow_idle_tool_entry
    );
    Err("Workflow was cancelled".to_string())
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
) -> Result<(), String> {
    let sessions = active_sessions.read().await;
    let Some(session) = sessions.get(session_id) else {
        return Ok(());
    };

    let mut expected_id = session.expected_response_id.write().await;
    validate_expected_response_id(session_id, expected_id.as_deref(), received_message_id)?;
    expected_id.take();
    Ok(())
}

pub(crate) fn validate_expected_response_id(
    session_id: &str,
    expected_response_id: Option<&str>,
    received_message_id: &str,
) -> Result<(), String> {
    let Some(expected_response_id) = expected_response_id else {
        log::info!(
            "Ignoring stray LLM response for session {} (received_message_id={}, expected_response_id=<none>)",
            session_id,
            received_message_id
        );
        return Err("LLM response superseded".to_string());
    };

    if received_message_id != expected_response_id {
        log::info!(
            "Ignoring superseded LLM response for session {} (expected_response_id={}, received_message_id={})",
            session_id,
            expected_response_id,
            received_message_id
        );
        return Err("LLM response superseded".to_string());
    }

    Ok(())
}
