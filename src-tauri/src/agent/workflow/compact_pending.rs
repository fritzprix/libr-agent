//! Guarantee pending user prompts survive compaction settle/resume.

use crate::agent::state::{AgentSession, CompactionResumeAction};
use crate::mcp::MCPServiceProxyManager;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionStatus;
use crate::services::MessageService;
use crate::state::get_message_repository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// How to continue after compaction settles when waiters may be queued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactPendingContinueKind {
    /// No pending continuation needed.
    None,
    /// Resume LLM completion (claims pending at turn start via `process_pending_messages`).
    ResumeCompletion,
    /// Session is Idle/Paused; start a workflow from the first queued prompt.
    StartFromPending,
}

/// Decide how to continue after compaction settles.
///
/// - Preflight `ResumeCompletion` always resumes (pending is claimed inside
///   `request_llm_completion` → `process_pending_messages`).
/// - Manual `Nothing` with waiters: only Idle/Paused need an explicit start.
///   Busy/Queued already have a turn that will claim pending via their own
///   lifecycle (`process_pending_messages` / finish-window continue).
pub fn compact_pending_continue_kind(
    resume_action: &CompactionResumeAction,
    status: SessionStatus,
    has_pending: bool,
) -> CompactPendingContinueKind {
    match resume_action {
        CompactionResumeAction::ResumeCompletion => CompactPendingContinueKind::ResumeCompletion,
        CompactionResumeAction::Nothing => {
            if !has_pending {
                return CompactPendingContinueKind::None;
            }
            match status {
                SessionStatus::Idle | SessionStatus::Paused => {
                    CompactPendingContinueKind::StartFromPending
                }
                SessionStatus::Busy
                | SessionStatus::Queued
                | SessionStatus::Error
                | SessionStatus::Provisioning => CompactPendingContinueKind::None,
            }
        }
    }
}

/// After compaction settles, continue so queued user prompts are not orphaned.
///
/// Preflight resume still spawns LLM completion. Manual settle only starts a
/// turn when the session is Idle/Paused with waiters; Busy/Queued leave
/// claiming to the existing workflow.
pub async fn continue_pending_after_compaction(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: &str,
    resume_action: CompactionResumeAction,
    spawn_resume_completion: impl FnOnce(),
) -> Result<(), String> {
    let (status, has_pending) = {
        let active = active_sessions.read().await;
        let Some(session) = active.get(session_id) else {
            return Ok(());
        };
        let status = session.metadata.status.clone();
        let has_pending = session.pending_events.read().await.count() > 0;
        (status, has_pending)
    };

    match compact_pending_continue_kind(&resume_action, status, has_pending) {
        CompactPendingContinueKind::None => Ok(()),
        CompactPendingContinueKind::ResumeCompletion => {
            log::info!(
                "▶️ Continuing after compaction via resumed LLM completion for session {} (pending={})",
                session_id,
                has_pending
            );
            spawn_resume_completion();
            Ok(())
        }
        CompactPendingContinueKind::StartFromPending => {
            log::info!(
                "▶️ Starting workflow from pending queue after manual compaction for session {}",
                session_id
            );
            start_workflow_from_pending_queue(
                session_repo,
                active_sessions,
                proxy_manager,
                app_handle,
                session_id,
            )
            .await
        }
    }
}

async fn start_workflow_from_pending_queue(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<(), String> {
    let pending_ids = {
        let sessions = active_sessions.read().await;
        let Some(session) = sessions.get(session_id) else {
            return Ok(());
        };
        let ids = session.pending_events.read().await.message_ids();
        ids
    };
    if pending_ids.is_empty() {
        return Ok(());
    }

    // Load bodies before draining so a lookup failure leaves the queue intact.
    let loaded_messages = get_message_repository()
        .get_by_ids(pending_ids.clone())
        .await
        .map_err(|error| error.to_string())?;
    let mut messages_by_id: HashMap<String, Message> = loaded_messages
        .into_iter()
        .map(|message| (message.id.clone(), message))
        .collect();

    let taken_ids =
        crate::agent::pending_queue::take_all_pending_message_ids(active_sessions, session_id)
            .await?;
    if taken_ids.is_empty() {
        return Ok(());
    }

    let ordered_messages: Vec<Message> = taken_ids
        .iter()
        .filter_map(|id| messages_by_id.remove(id))
        .collect();
    if ordered_messages.is_empty() {
        return Ok(());
    }

    let mut messages = ordered_messages.into_iter();
    let Some(first) = messages.next() else {
        return Ok(());
    };
    let rest: Vec<Message> = messages.collect();

    if let Err(error) = crate::agent::workflow::start_workflow(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id.to_string(),
        first.clone(),
    )
    .await
    {
        let mut to_restore = vec![first];
        to_restore.extend(rest);
        crate::agent::pending_queue::restore_pending_messages_after_take(
            active_sessions,
            session_id,
            &to_restore,
        )
        .await;
        return Err(error);
    }

    for (index, message) in rest.iter().enumerate() {
        if let Err(error) =
            MessageService::queue_user_message(active_sessions, app_handle, session_id, message)
                .await
        {
            let to_restore = rest[index..].to_vec();
            crate::agent::pending_queue::restore_pending_messages_after_take(
                active_sessions,
                session_id,
                &to_restore,
            )
            .await;
            return Err(error);
        }
    }

    let _ = crate::agent::pending_queue::emit_pending_queue_updated(
        active_sessions,
        app_handle,
        session_id,
    )
    .await;

    Ok(())
}
