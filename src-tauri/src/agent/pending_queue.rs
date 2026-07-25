//! Durable FIFO waiting prompts: messages table + thin `pending_queue` index.

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
        if pending
            .message_ids()
            .iter()
            .any(|id| id == &user_message.id)
        {
            return Ok(());
        }
        pending.add(crate::agent::state::PendingEvent::Message(
            user_message.id.clone(),
        ));
    }

    let message_repo = get_message_repository();
    if let Err(e) = message_repo.insert(user_message).await {
        // Roll back in-memory enqueue on DB failure.
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
        // Best-effort cleanup of the orphaned message row.
        let _ = message_repo.delete_by_id(&user_message.id).await;
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

/// Promote the next waiting prompt into the active message cache (FIFO).
///
/// Memory is drained first for a short critical section, then durable index
/// removal runs. On durable failure the id is restored to the FIFO front so
/// the in-session queue stays consistent with SQLite.
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
        let claimed = session.pending_events.write().await.drain_one_message();
        claimed
    };

    let Some(message_id) = message_id else {
        return Ok(None);
    };

    if let Err(e) = get_pending_queue_repository().remove(&message_id).await {
        restore_front_pending_message(active_sessions, session_id, message_id).await;
        return Err(e.to_string());
    }

    let repo = get_message_repository();
    let messages = match repo.get_by_ids(vec![message_id.clone()]).await {
        Ok(messages) => messages,
        Err(e) => {
            // Index already dropped; re-enqueue so hydrate can recover later,
            // and restore memory so the live session still sees the prompt.
            let created_at = chrono::Utc::now().timestamp_millis();
            if let Err(requeue_err) = get_pending_queue_repository()
                .enqueue(session_id, &message_id, created_at)
                .await
            {
                log::error!(
                    "Failed to re-enqueue pending index for {message_id} after load error: {requeue_err}"
                );
            }
            restore_front_pending_message(active_sessions, session_id, message_id).await;
            return Err(e.to_string());
        }
    };
    let Some(message) = messages.into_iter().next() else {
        log::warn!(
            "Pending message {message_id} missing from DB for session {session_id}; skipping"
        );
        let _ = emit_pending_queue_updated(active_sessions, app_handle, session_id).await;
        return Ok(None);
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

    let event = AgentEvent::MessageAdded {
        session_id: session_id.to_string(),
        message: Box::new(message.clone()),
    };
    crate::agent::tauri_events::emit_agent_event(app_handle, event)
        .map_err(|e| format!("Failed to emit MessageAdded: {e}"))?;

    emit_pending_queue_updated(active_sessions, app_handle, session_id).await?;
    Ok(Some(message))
}

/// Cancel a waiting prompt. Durable deletes run before memory mutation; on
/// durable failure the in-memory queue is left unchanged (or the index is restored).
pub async fn cancel_pending_message(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    message_id: &str,
) -> Result<bool, String> {
    {
        let sessions = active_sessions.read().await;
        let Some(session) = sessions.get(session_id) else {
            return Err(format!("Session not found: {session_id}"));
        };
        if !session
            .pending_events
            .read()
            .await
            .contains_message(message_id)
        {
            return Ok(false);
        }
    }

    // Prefer the persisted timestamp so a mid-cancel index restore keeps FIFO order.
    let created_at = {
        let loaded = get_message_repository()
            .get_by_ids(vec![message_id.to_string()])
            .await
            .map_err(|e| e.to_string())?;
        loaded
            .into_iter()
            .next()
            .map(|m| m.created_at)
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis())
    };

    if let Err(e) = get_pending_queue_repository().remove(message_id).await {
        return Err(e.to_string());
    }

    if let Err(e) = get_message_repository().delete_by_id(message_id).await {
        if let Err(requeue_err) = get_pending_queue_repository()
            .enqueue(session_id, message_id, created_at)
            .await
        {
            log::error!(
                "Failed to restore pending index for {message_id} after delete error: {requeue_err}"
            );
        }
        return Err(format!("Failed to delete cancelled pending message: {e}"));
    }

    {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            session
                .pending_events
                .write()
                .await
                .remove_message(message_id);
        }
    }

    emit_pending_queue_updated(active_sessions, app_handle, session_id).await?;
    Ok(true)
}

async fn restore_front_pending_message(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    message_id: String,
) {
    let sessions = active_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        let mut pending = session.pending_events.write().await;
        if !pending.contains_message(&message_id) {
            pending.restore_front_message(message_id);
        }
    }
}

/// Drop all waiting prompts (terminate / hard clear). Soft cancel preserves them.
pub async fn discard_all_pending_messages(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: Option<&AppHandle>,
    session_id: &str,
) -> Result<(), String> {
    let message_ids = {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            let drained = session.pending_events.write().await.drain_messages();
            drained
        } else {
            Vec::new()
        }
    };

    let index_ids = get_pending_queue_repository()
        .remove_all_for_session(session_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut delete_set: HashSet<String> = message_ids.into_iter().collect();
    delete_set.extend(index_ids);

    let repo = get_message_repository();
    for msg_id in &delete_set {
        if let Err(e) = repo.delete_by_id(msg_id).await {
            log::error!("Failed to delete pending message {msg_id}: {e}");
        }
    }

    // Ensure waiting prompts are not left in the in-memory cache.
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
    let entries = get_pending_queue_repository()
        .list_by_session(session_id)
        .await
        .map_err(|e| e.to_string())?;

    let pending_ids: HashSet<String> = entries.iter().map(|e| e.message_id.clone()).collect();
    messages.retain(|m| !pending_ids.contains(&m.id));

    let mut pending = session.pending_events.write().await;
    pending.clear();
    for entry in entries {
        pending.add(crate::agent::state::PendingEvent::Message(entry.message_id));
    }

    Ok(())
}
