use super::AgentSessionManager;
use crate::agent::events::AgentEventDispatcher;
use crate::agent::state::AgentSession;
use crate::repositories::{CompactContextRecord, SessionRepository};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;

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
            (
                session.compact_started_at_ms.clone(),
                clear_last_compacted_tail_id.then(|| session.last_compacted_tail_id.clone()),
            )
        })
    };

    let Some((compact_started_at_ms_handle, last_compacted_tail_id_handle)) = handles else {
        return;
    };

    if let Some(last_compacted_tail_id_handle) = last_compacted_tail_id_handle {
        *last_compacted_tail_id_handle.write().await = None;
    }

    *compact_started_at_ms_handle.write().await = None;
}

pub async fn clear_compact_in_flight(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) {
    clear_compaction_state(active_sessions, session_id, false).await;
}

pub async fn handle_compact_error_with_dispatcher(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    dispatcher: &dyn AgentEventDispatcher,
    session_id: String,
    error: crate::agent::llm::types::AgentRuntimeError,
) -> Result<(), String> {
    let (was_awaiting, session_name, compact_started_at_ms_handle) = {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            (
                session.awaiting_compact_completion.load(Ordering::SeqCst),
                session
                    .metadata
                    .name
                    .clone()
                    .unwrap_or_else(|| session_id.chars().take(8).collect::<String>()),
                Some(session.compact_started_at_ms.clone()),
            )
        } else {
            (false, session_id.chars().take(8).collect::<String>(), None)
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

    let state_event = crate::agent::llm::types::CompactStateEvent {
        session_id: session_id.clone(),
        session_name: Some(session_name),
        compacting: false,
        phase: crate::agent::llm::types::CompactStatePhase::Failed,
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

    if was_awaiting {
        log::warn!(
            "Preflight compaction failed for session {}. Failing workflow.",
            session_id
        );
        crate::agent::llm::finalize_workflow_error_with_dispatcher(
            session_repo,
            active_sessions,
            dispatcher,
            session_id,
            error,
        )
        .await?;
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

    let should_resume_completion = {
        let active = manager.active_sessions.read().await;
        active
            .get(session_id)
            .map(|session| {
                session
                    .awaiting_compact_completion
                    .swap(false, Ordering::SeqCst)
            })
            .unwrap_or(false)
    };

    log::info!(
        "📌 Compact completion decision for session {}: should_resume_completion={}",
        session_id,
        should_resume_completion
    );

    clear_compact_in_flight(&manager.active_sessions, session_id).await;

    if should_resume_completion {
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
    }

    Ok(())
}
