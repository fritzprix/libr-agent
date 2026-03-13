use crate::agent::state::AgentSession;
use crate::repositories::MessageRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelStrategy {
    DeferToMessageBoundary,
    StopImmediately,
}

pub fn classify_cancel_strategy(has_pending_execution: bool) -> CancelStrategy {
    if has_pending_execution {
        CancelStrategy::DeferToMessageBoundary
    } else {
        CancelStrategy::StopImmediately
    }
}

pub fn should_consume_cancel_at_message_boundary(cancel_pending: bool) -> bool {
    cancel_pending
}

pub async fn discard_pending_events(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
) {
    let mut messages_to_delete = Vec::new();

    // 1. Drain from pending events queue and remove from in-memory cache
    {
        let sessions = active_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            let mut pending_events = session.pending_events.write().await;
            messages_to_delete = pending_events.drain_messages();

            if !messages_to_delete.is_empty() {
                let mut messages = session.messages.write().await;
                // Remove these messages from the cache
                messages.retain(|m| !messages_to_delete.contains(&m.id));

                log::info!(
                    "Cleared {} pending events from queue and cache for session {}",
                    messages_to_delete.len(),
                    session_id
                );
            }
        }
    }

    // 2. Delete from database
    if !messages_to_delete.is_empty() {
        let repo = crate::state::get_message_repository();
        for msg_id in messages_to_delete {
            if let Err(e) = repo.delete_by_id(&msg_id).await {
                log::error!(
                    "Failed to delete cancelled pending message {} from DB: {}",
                    msg_id,
                    e
                );
            }
        }
    }
}
