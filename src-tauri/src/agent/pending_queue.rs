//! Durable FIFO waiting prompts: messages table + thin `pending_queue` index.

use crate::agent::events::AgentEvent;
use crate::agent::state::AgentSession;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::pending_queue_repository::{PendingQueueEntry, PendingQueueRepository};
use crate::state::{get_message_repository, get_pending_queue_repository};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

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

async fn load_messages_by_ids(ids: Vec<String>) -> Result<Vec<Message>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let repo = get_message_repository();
    repo.get_by_ids(ids).await.map_err(|e| e.to_string())
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
        if let Err(cleanup_err) = message_repo.delete_by_id(&user_message.id).await {
            log::error!(
                "Failed to delete orphaned queued message {}: {cleanup_err}",
                user_message.id
            );
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

enum PromotePendingOutcome {
    Promoted(Box<Message>),
    Skipped,
}

struct PromotePendingFailure {
    error: String,
    index_entry: Option<PendingQueueEntry>,
}

async fn promote_pending_message_id(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    message_id: String,
) -> Result<PromotePendingOutcome, PromotePendingFailure> {
    let index_entry = match get_pending_queue_repository()
        .remove_returning(&message_id)
        .await
    {
        Ok(entry) => entry,
        Err(e) => {
            return Err(PromotePendingFailure {
                error: e.to_string(),
                index_entry: None,
            });
        }
    };

    let Some(index_entry) = index_entry else {
        log::warn!(
            "Pending index missing for claimed message {message_id} in session {session_id}; skipping"
        );
        return Ok(PromotePendingOutcome::Skipped);
    };

    let repo = get_message_repository();
    let messages = match repo.get_by_ids(vec![message_id.clone()]).await {
        Ok(messages) => messages,
        Err(e) => {
            return Err(PromotePendingFailure {
                error: e.to_string(),
                index_entry: Some(index_entry),
            });
        }
    };
    let Some(message) = messages.into_iter().next() else {
        log::warn!(
            "Pending message {message_id} missing from DB for session {session_id}; skipping"
        );
        return Ok(PromotePendingOutcome::Skipped);
    };

    {
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

    Ok(PromotePendingOutcome::Promoted(Box::new(message)))
}

async fn emit_message_added(
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

/// Promote the next waiting prompt into the active message cache (FIFO).
///
/// Memory is drained first, then the durable index row is removed and returned
/// so failure recovery can restore the original `queue_seq` / `created_at`.
pub async fn claim_next_pending_message(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<Option<Message>, String> {
    let message_id = {
        let sessions = active_sessions.read().await;
        let Some(session) = sessions.get(session_id) else {
            return Ok(None);
        };
        let drained = session.pending_events.write().await.drain_one_message();
        drained
    };

    let Some(message_id) = message_id else {
        return Ok(None);
    };

    let promoted =
        match promote_pending_message_id(active_sessions, session_id, message_id.clone()).await {
            Ok(outcome) => outcome,
            Err(failure) => {
                if let Some(index_entry) = failure.index_entry {
                    restore_index_entry(&index_entry).await;
                }
                restore_front_pending_message(active_sessions, session_id, message_id).await;
                return Err(failure.error);
            }
        };

    let PromotePendingOutcome::Promoted(message) = promoted else {
        let _ = emit_pending_queue_updated(active_sessions, app_handle, session_id).await;
        return Ok(None);
    };

    emit_message_added(app_handle, session_id, &message).await?;
    emit_pending_queue_updated(active_sessions, app_handle, session_id).await?;
    Ok(Some(*message))
}

/// Promote every waiting prompt into the active message cache in one LLM turn (FIFO).
pub async fn claim_all_pending_messages(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<Vec<Message>, String> {
    let message_ids = {
        let sessions = active_sessions.read().await;
        let Some(session) = sessions.get(session_id) else {
            return Ok(Vec::new());
        };
        let drained = session.pending_events.write().await.drain_messages();
        drained
    };

    if message_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut promoted_messages = Vec::new();

    for (index, message_id) in message_ids.iter().enumerate() {
        match promote_pending_message_id(active_sessions, session_id, message_id.clone()).await {
            Ok(PromotePendingOutcome::Promoted(message)) => {
                emit_message_added(app_handle, session_id, &message).await?;
                promoted_messages.push(*message);
            }
            Ok(PromotePendingOutcome::Skipped) => {}
            Err(failure) => {
                if let Some(index_entry) = failure.index_entry {
                    restore_index_entry(&index_entry).await;
                }
                restore_front_pending_messages(active_sessions, session_id, &message_ids[index..])
                    .await;
                return Err(failure.error);
            }
        }
    }

    emit_pending_queue_updated(active_sessions, app_handle, session_id).await?;
    Ok(promoted_messages)
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

async fn restore_index_entry(entry: &PendingQueueEntry) {
    if let Err(e) = get_pending_queue_repository()
        .enqueue_with_seq(
            &entry.session_id,
            &entry.message_id,
            entry.created_at,
            entry.queue_seq,
        )
        .await
    {
        log::error!(
            "Failed to restore pending index for {} (seq={}): {e}",
            entry.message_id,
            entry.queue_seq
        );
    }
}

async fn restore_front_pending_message(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    message_id: String,
) {
    restore_front_pending_messages(active_sessions, session_id, &[message_id]).await;
}

async fn restore_front_pending_messages(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    message_ids: &[String],
) {
    if message_ids.is_empty() {
        return;
    }

    let sessions = active_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        let mut pending = session.pending_events.write().await;
        let missing_ids: Vec<String> = message_ids
            .iter()
            .filter(|id| !pending.contains_message(id))
            .cloned()
            .collect();
        pending.restore_front_pending_messages(&missing_ids);
    }
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

/// Rebuild in-memory pending_events from the durable index and strip those rows from cache.
pub async fn hydrate_pending_queue_into_session(
    session: &AgentSession,
    session_id: &str,
    messages: &mut Vec<Message>,
) -> Result<(), String> {
    let queue_repo = get_pending_queue_repository();
    if let Err(e) = queue_repo.delete_orphans_for_session(session_id).await {
        log::warn!("Failed to purge orphan pending_queue rows for {session_id}: {e}");
    }

    let entries = queue_repo
        .list_by_session(session_id)
        .await
        .map_err(|e| e.to_string())?;

    let pending_ids: Vec<String> = entries.iter().map(|e| e.message_id.clone()).collect();
    let existing = if pending_ids.is_empty() {
        Vec::new()
    } else {
        get_message_repository()
            .get_by_ids(pending_ids.clone())
            .await
            .map_err(|e| e.to_string())?
    };
    let existing_ids: HashSet<String> = existing.into_iter().map(|m| m.id).collect();

    let valid_ids: Vec<String> = pending_ids
        .into_iter()
        .filter(|id| existing_ids.contains(id))
        .collect();

    let valid_set: HashSet<String> = valid_ids.iter().cloned().collect();
    messages.retain(|m| !valid_set.contains(&m.id));

    let mut pending = session.pending_events.write().await;
    pending.clear();
    for id in valid_ids {
        pending.add(crate::agent::state::PendingEvent::Message(id));
    }

    Ok(())
}
