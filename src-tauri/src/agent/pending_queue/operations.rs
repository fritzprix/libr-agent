use crate::agent::events::AgentEvent;
use crate::agent::state::AgentSession;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::pending_queue_repository::PendingQueueRepository;
use crate::state::{get_message_repository, get_pending_queue_repository};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::restore::{
    load_messages_by_ids, restore_front_pending_message, restore_front_pending_messages,
};

pub async fn emit_pending_queue_updated(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<(), String> {
    let messages = list_pending_messages(active_sessions, session_id).await?;
    let event = AgentEvent::PendingQueueUpdated {
        session_id: session_id.to_string(),
        messages,
    };
    crate::agent::tauri_events::emit_agent_event(app_handle, event)
        .map_err(|e| format!("Failed to emit PendingQueueUpdated: {e}"))
}

pub(crate) async fn emit_message_added(
    app_handle: &AppHandle,
    session_id: &str,
    message: &Message,
) -> Result<(), String> {
    let event = AgentEvent::MessageAdded {
        session_id: session_id.to_string(),
        message: Box::new(message.clone()),
    };
    crate::agent::tauri_events::emit_agent_event(app_handle, event)
        .map_err(|e| format!("Failed to emit MessageAdded: {e}"))
}

pub(crate) async fn push_message_to_session_cache(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    message: &Message,
) {
    let sessions = active_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        let mut cache = session.messages.write().await;
        if !cache.iter().any(|m| m.id == message.id) {
            cache.push(message.clone());
            if cache.len() > crate::agent::state::MAX_CACHED_MESSAGES {
                cache.remove(0);
            }
        }
    }
}

pub async fn list_pending_messages(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<Vec<Message>, String> {
    let cached_ids = {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            let ids = session.pending_events.read().await.message_ids();
            Some(ids)
        } else {
            None
        }
    };

    let ids = match cached_ids {
        Some(ids) => ids,
        None => {
            // Session may not be active yet; fall back to durable index.
            let entries = get_pending_queue_repository()
                .list_by_session(session_id)
                .await
                .map_err(|e| e.to_string())?;
            entries.into_iter().map(|e| e.message_id).collect()
        }
    };

    load_messages_by_ids(ids).await
}

/// Persist a waiting user prompt without touching the active LLM context cache.
pub async fn enqueue_pending_user_message(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    user_message: &Message,
) -> Result<(), String> {
    {
        let sessions = active_sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {session_id}"))?;
        let mut pending = session.pending_events.write().await;
        if pending.contains_message(&user_message.id) {
            return Ok(());
        }
        pending.add(crate::agent::state::PendingEvent::Message(
            user_message.id.clone(),
        ));
    }

    let message_repo = get_message_repository();
    // Only delete the messages-table row on index failure when this call created it.
    // Re-queue of an existing body must never destroy the durable message.
    let body_already_existed = match message_repo.get_by_ids(vec![user_message.id.clone()]).await {
        Ok(existing) => existing.into_iter().any(|row| row.id == user_message.id),
        Err(e) => {
            if let Some(session) = active_sessions.read().await.get(session_id) {
                session
                    .pending_events
                    .write()
                    .await
                    .remove_message(&user_message.id);
            }
            return Err(format!("Failed to check queued message existence: {e}"));
        }
    };

    if let Err(e) = message_repo.insert(user_message).await {
        if let Some(session) = active_sessions.read().await.get(session_id) {
            session
                .pending_events
                .write()
                .await
                .remove_message(&user_message.id);
        }
        return Err(format!("Failed to persist queued message: {e}"));
    }

    if let Err(e) = get_pending_queue_repository()
        .enqueue(session_id, &user_message.id, user_message.created_at)
        .await
    {
        if !body_already_existed {
            if let Err(cleanup_err) = message_repo.delete_by_id(&user_message.id).await {
                log::error!(
                    "Failed to delete orphaned queued message {}: {cleanup_err}",
                    user_message.id
                );
            }
        }
        if let Some(session) = active_sessions.read().await.get(session_id) {
            session
                .pending_events
                .write()
                .await
                .remove_message(&user_message.id);
        }
        return Err(format!("Failed to persist pending queue index: {e}"));
    }

    emit_pending_queue_updated(active_sessions, app_handle, session_id).await?;
    Ok(())
}

/// Cancel a waiting prompt.
///
/// Takes ownership from the in-memory queue first so a concurrent claim cannot
/// promote the same id into the LLM cache while this path deletes the DB row
/// (TOCTOU). Durable delete runs in a transaction; on failure memory is restored.
pub async fn cancel_pending_message(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    message_id: &str,
) -> Result<bool, String> {
    let owned = {
        let sessions = active_sessions.read().await;
        let Some(session) = sessions.get(session_id) else {
            return Err(format!("Session not found: {session_id}"));
        };
        let removed = session
            .pending_events
            .write()
            .await
            .remove_message(message_id);
        removed
    };

    if !owned {
        // Already claimed or cancelled — do not delete a promoted message.
        return Ok(false);
    }

    match get_pending_queue_repository()
        .remove_index_and_message(message_id)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            // Index already gone; still try to drop a leftover message row.
            if let Err(e) = get_message_repository().delete_by_id(message_id).await {
                log::warn!("Cancel found no pending index for {message_id}; message delete: {e}");
            }
        }
        Err(e) => {
            restore_front_pending_message(active_sessions, session_id, message_id.to_string())
                .await;
            return Err(format!("Failed to delete cancelled pending message: {e}"));
        }
    }

    emit_pending_queue_updated(active_sessions, app_handle, session_id).await?;
    Ok(true)
}

/// Take specific waiting message IDs out of memory and the durable index without
/// deleting message bodies or promoting into the active cache.
///
/// Only the provided IDs are removed; any waiters that arrived after the caller's
/// snapshot stay queued. Callers that fail after this drain must restore via
/// [`restore_pending_messages_after_take`].
pub async fn take_pending_message_ids(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    message_ids: &[String],
) -> Result<Vec<String>, String> {
    if message_ids.is_empty() {
        return Ok(Vec::new());
    }

    let taken_ids = {
        let sessions = active_sessions.read().await;
        let Some(session) = sessions.get(session_id) else {
            return Ok(Vec::new());
        };
        let mut pending = session.pending_events.write().await;
        let mut taken = Vec::new();
        for message_id in message_ids {
            if pending.remove_message(message_id) {
                taken.push(message_id.clone());
            }
        }
        taken
    };

    if taken_ids.is_empty() {
        return Ok(Vec::new());
    }

    let queue_repo = get_pending_queue_repository();
    let mut removed_ok: Vec<String> = Vec::new();
    for message_id in &taken_ids {
        match queue_repo.remove(message_id).await {
            Ok(()) => removed_ok.push(message_id.clone()),
            Err(error) => {
                log::error!(
                    "Failed to clear pending_queue index for taken message {} (session {}): {}",
                    message_id,
                    session_id,
                    error
                );
                restore_front_pending_messages(active_sessions, session_id, &taken_ids).await;
                let created_at = chrono::Utc::now().timestamp_millis();
                for restored_id in &removed_ok {
                    if let Err(restore_error) = queue_repo
                        .enqueue(session_id, restored_id, created_at)
                        .await
                    {
                        log::error!(
                            "Failed to restore pending_queue index for {} (session {}) after take failure: {}",
                            restored_id,
                            session_id,
                            restore_error
                        );
                    }
                }
                return Err(format!(
                    "Failed to clear pending_queue index for {message_id}: {error}"
                ));
            }
        }
    }

    Ok(taken_ids)
}

/// Take all waiting message IDs out of memory and the durable index without
/// deleting message bodies or promoting into the active cache.
///
/// Used when starting a new workflow from queued prompts after Manual
/// compaction settles on an Idle/Paused session. Callers that fail after this
/// drain must restore via [`restore_pending_messages_after_take`].
pub async fn take_all_pending_message_ids(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) -> Result<Vec<String>, String> {
    let message_ids = {
        let sessions = active_sessions.read().await;
        let Some(session) = sessions.get(session_id) else {
            return Ok(Vec::new());
        };
        let ids = session.pending_events.read().await.message_ids();
        ids
    };
    take_pending_message_ids(active_sessions, session_id, &message_ids).await
}

/// Drop all waiting prompts (terminate / hard clear). Soft cancel preserves them.
pub async fn discard_all_pending_messages(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: Option<&AppHandle>,
    session_id: &str,
) -> Result<(), String> {
    let (message_ids, protected_ids) = {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            let pending_ids = session.pending_events.write().await.drain_messages();
            // Promoted prompts may still linger in pending_queue after docker
            // drain/start_workflow. Never delete ids already in the active
            // transcript cache — that is the Session-API first-bubble loss bug.
            let protected_ids: HashSet<String> = session
                .messages
                .read()
                .await
                .iter()
                .map(|message| message.id.clone())
                .collect();
            (pending_ids, protected_ids)
        } else {
            (Vec::new(), HashSet::new())
        }
    };

    let index_ids = get_pending_queue_repository()
        .remove_all_for_session(session_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut delete_set: HashSet<String> = message_ids.into_iter().collect();
    delete_set.extend(index_ids);
    delete_set.retain(|message_id| !protected_ids.contains(message_id));

    let repo = get_message_repository();
    for msg_id in &delete_set {
        if let Err(e) = repo.delete_by_id(msg_id).await {
            log::error!("Failed to delete pending message {msg_id}: {e}");
        }
    }

    {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            let mut messages = session.messages.write().await;
            messages.retain(|m| !delete_set.contains(&m.id));
        }
    }

    if let Some(app_handle) = app_handle {
        emit_pending_queue_updated(active_sessions, app_handle, session_id).await?;
    }

    Ok(())
}
