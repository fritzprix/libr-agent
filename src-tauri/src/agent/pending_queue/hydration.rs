use crate::agent::state::AgentSession;
use crate::models::chat::Message;
use crate::repositories::pending_queue_repository::PendingQueueRepository;
use crate::repositories::{MessageRepository, SessionStatus};
use crate::state::{get_message_repository, get_pending_queue_repository};
use std::collections::HashSet;

/// Last non-recovery message when it is still an unanswered user turn.
///
/// Idle session-start / incomplete-turn prompts belong on the message stack.
/// A lingering `pending_queue` row for that tip is stale and must not re-queue.
pub fn incomplete_turn_user_id(messages: &[Message]) -> Option<&str> {
    messages
        .iter()
        .rfind(|m| !m.is_recovery_message())
        .filter(|m| m.role == "user")
        .map(|m| m.id.as_str())
}

/// Load durable pending IDs that still resolve to real messages (FIFO).
pub async fn load_valid_pending_message_ids(session_id: &str) -> Result<Vec<String>, String> {
    let queue_repo = get_pending_queue_repository();
    if let Err(e) = queue_repo.delete_orphans_for_session(session_id).await {
        log::warn!("Failed to purge orphan pending_queue rows for {session_id}: {e}");
    }

    let entries = queue_repo
        .list_by_session(session_id)
        .await
        .map_err(|e| e.to_string())?;

    let pending_ids: Vec<String> = entries.iter().map(|e| e.message_id.clone()).collect();
    if pending_ids.is_empty() {
        return Ok(Vec::new());
    }

    let existing = get_message_repository()
        .get_by_ids(pending_ids.clone())
        .await
        .map_err(|e| e.to_string())?;
    let existing_ids: HashSet<String> = existing.into_iter().map(|m| m.id).collect();

    Ok(pending_ids
        .into_iter()
        .filter(|id| existing_ids.contains(id))
        .collect())
}

/// Classification of durable `pending_queue` rows against a transcript/cache slice.
///
/// Promoted prompts may linger in `pending_queue` after docker drain /
/// `start_workflow`. Those IDs must stay on the active message stack and must
/// not be re-queued into `pending_events`.
struct PendingQueueClassification {
    /// True waiters: strip from transcript/cache; rebuild `pending_events`.
    waiting_ids: Vec<String>,
    /// Stale linger: already answered or protected; keep in slice.
    stale_ids: Vec<String>,
}

/// Split durable pending IDs into true waiters vs stale linger rows.
///
/// - A pending ID that already has a later non-recovery assistant reply is stale.
/// - A pending ID listed in `protect_ids` (live stack / Idle incomplete tip) is stale.
/// - Remaining pending IDs are true waiters (including IDs not present in the slice).
fn classify_pending_queue_ids(
    messages: &[Message],
    pending_ids: &[String],
    protect_ids: &HashSet<String>,
) -> PendingQueueClassification {
    let pending_set: HashSet<&str> = pending_ids.iter().map(String::as_str).collect();
    let mut open_without_reply: HashSet<String> = HashSet::new();
    let mut stale: HashSet<String> = HashSet::new();

    for message in messages {
        if message.role == "assistant" && !message.is_recovery_message() {
            stale.extend(open_without_reply.drain());
        }
        if pending_set.contains(message.id.as_str()) {
            open_without_reply.insert(message.id.clone());
        }
    }

    for id in pending_ids {
        if protect_ids.contains(id) {
            stale.insert(id.clone());
            open_without_reply.remove(id);
        }
    }

    let slice_ids: HashSet<&str> = messages.iter().map(|m| m.id.as_str()).collect();
    let waiting_ids: Vec<String> = pending_ids
        .iter()
        .filter(|id| {
            if stale.contains(*id) {
                return false;
            }
            open_without_reply.contains(*id) || !slice_ids.contains(id.as_str())
        })
        .cloned()
        .collect();

    let stale_ids: Vec<String> = pending_ids
        .iter()
        .filter(|id| stale.contains(*id))
        .cloned()
        .collect();

    PendingQueueClassification {
        waiting_ids,
        stale_ids,
    }
}

async fn purge_stale_pending_index_rows(session_id: &str, stale_ids: &[String]) {
    if stale_ids.is_empty() {
        return;
    }

    let queue_repo = get_pending_queue_repository();
    for message_id in stale_ids {
        if let Err(error) = queue_repo.remove(message_id).await {
            log::warn!(
                "Failed to purge stale pending_queue row {message_id} for session {session_id}: {error}"
            );
        } else {
            log::info!(
                "Purged stale pending_queue row {message_id} for session {session_id} (already on active stack / answered)"
            );
        }
    }
}

/// Remove true waiting prompts from a message list (UI / cache slices).
///
/// Uses the durable `pending_queue` index. Never strips IDs in `protect_ids`
/// (live active message stack / Idle incomplete tip).
///
/// Returns the true waiting IDs (FIFO), suitable for rebuilding `pending_events`.
pub async fn strip_pending_queue_messages(
    session_id: &str,
    messages: &mut Vec<Message>,
) -> Result<Vec<String>, String> {
    strip_pending_queue_messages_with_protect(session_id, messages, &HashSet::new()).await
}

/// Same as [`strip_pending_queue_messages`], with an explicit protect set.
pub async fn strip_pending_queue_messages_with_protect(
    session_id: &str,
    messages: &mut Vec<Message>,
    protect_ids: &HashSet<String>,
) -> Result<Vec<String>, String> {
    let valid_ids = load_valid_pending_message_ids(session_id).await?;
    if valid_ids.is_empty() {
        return Ok(valid_ids);
    }

    let classification = classify_pending_queue_ids(messages, &valid_ids, protect_ids);
    purge_stale_pending_index_rows(session_id, &classification.stale_ids).await;

    if classification.waiting_ids.is_empty() {
        return Ok(classification.waiting_ids);
    }

    let waiting_set: HashSet<&str> = classification
        .waiting_ids
        .iter()
        .map(String::as_str)
        .collect();
    messages.retain(|message| !waiting_set.contains(message.id.as_str()));
    Ok(classification.waiting_ids)
}

/// Rebuild in-memory pending_events from true waiters and strip those rows from cache.
///
/// Idle / already-promoted messages stay on the message stack. Only durable
/// waiting prompts are moved into `pending_events` so the workflow loop can
/// dequeue them via `claim_all_pending_messages` at the next LLM turn.
///
/// When the session is Idle/Paused, the incomplete-turn tip is protected even
/// if it still has a stale `pending_queue` row — session-start requests must
/// remain on the message stack (never re-enter pending).
pub async fn hydrate_pending_queue_into_session(
    session: &AgentSession,
    session_id: &str,
    messages: &mut Vec<Message>,
) -> Result<(), String> {
    let mut protect_ids = HashSet::new();
    if matches!(
        session.metadata.status,
        SessionStatus::Idle | SessionStatus::Paused
    ) {
        if let Some(tip_id) = incomplete_turn_user_id(messages) {
            protect_ids.insert(tip_id.to_string());
        }
    }

    let waiting_ids =
        strip_pending_queue_messages_with_protect(session_id, messages, &protect_ids).await?;

    let mut pending = session.pending_events.write().await;
    pending.clear();
    for id in waiting_ids {
        pending.add(crate::agent::state::PendingEvent::Message(id));
    }

    Ok(())
}
