use crate::agent::state::AgentSession;
use crate::models::chat::Message;
use crate::repositories::pending_queue_repository::{PendingQueueEntry, PendingQueueRepository};
use crate::repositories::MessageRepository;
use crate::state::{get_message_repository, get_pending_queue_repository};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(crate) async fn load_messages_by_ids(ids: Vec<String>) -> Result<Vec<Message>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let repo = get_message_repository();
    repo.get_by_ids(ids).await.map_err(|e| e.to_string())
}

pub(crate) async fn restore_index_entry(entry: &PendingQueueEntry) {
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

pub(crate) async fn restore_front_pending_message(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    message_id: String,
) {
    restore_front_pending_messages(active_sessions, session_id, &[message_id]).await;
}

pub(crate) async fn restore_front_pending_messages(
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

/// Restore memory + durable index after [`take_all_pending_message_ids`] when a
/// subsequent workflow start fails, so waiters are not permanently dropped.
pub async fn restore_pending_messages_after_take(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    messages: &[Message],
) {
    if messages.is_empty() {
        return;
    }

    let message_ids: Vec<String> = messages.iter().map(|message| message.id.clone()).collect();
    restore_front_pending_messages(active_sessions, session_id, &message_ids).await;

    let queue_repo = get_pending_queue_repository();
    for message in messages {
        if let Err(error) = queue_repo
            .enqueue(session_id, &message.id, message.created_at)
            .await
        {
            log::error!(
                "Failed to restore pending_queue index for {} (session {}): {}",
                message.id,
                session_id,
                error
            );
        }
    }
}
