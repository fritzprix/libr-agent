//! Durable FIFO waiting prompts: messages table + thin `pending_queue` index.
//!
//! Routing invariants:
//! 1. Idle / session-start user request → append onto the active message stack and
//!    start the workflow (`start_workflow`). Must not enter `pending_queue`.
//! 2. Busy / Queued / Provisioning → enqueue into `pending_events` + durable index
//!    only (not the active stack). The workflow loop dequeues via
//!    `claim_all_pending_messages` at the start of each LLM turn.
//! 3. Workflow finish → if waiters remain, continue the loop (claim on next turn)
//!    rather than going Idle with an orphaned queue. Cancel may discard instead.

use crate::agent::events::AgentEvent;
use crate::agent::message_merge::{merge_user_message_attachments, merge_user_message_contents};
use crate::agent::state::AgentSession;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::pending_queue_repository::{PendingQueueEntry, PendingQueueRepository};
use crate::repositories::SessionStatus;
use crate::state::{get_message_repository, get_pending_queue_repository};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Max waiting prompts claimed and merged into one user message per LLM turn.
/// Remaining FIFO items stay queued for the next turn.
pub const MAX_PENDING_CLAIM_BATCH: usize = 8;

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

async fn push_message_to_session_cache(
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

/// Promote every waiting prompt into the active message cache in one LLM turn (FIFO).
/// Multiple pending user messages are merged into a single user message to avoid
/// consecutive user messages in the session history.
///
/// At most [`MAX_PENDING_CLAIM_BATCH`] messages are claimed per call; remainder
/// stays queued. Durable merge (upsert keeper + delete absorbed + clear index)
/// runs in one DB transaction so failure leaves the queue intact.
pub async fn claim_all_pending_messages(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
) -> Result<Vec<Message>, String> {
    let pending_events = {
        let sessions = active_sessions.read().await;
        sessions
            .get(session_id)
            .map(|s| Arc::clone(&s.pending_events))
    };
    let Some(pending_events) = pending_events else {
        return Ok(Vec::new());
    };
    let all_message_ids = pending_events.write().await.drain_messages();

    if all_message_ids.is_empty() {
        return Ok(Vec::new());
    }

    let (claim_ids, remainder_ids) = if all_message_ids.len() > MAX_PENDING_CLAIM_BATCH {
        let (claimed, remainder) = all_message_ids.split_at(MAX_PENDING_CLAIM_BATCH);
        (claimed.to_vec(), remainder.to_vec())
    } else {
        (all_message_ids, Vec::new())
    };

    // Keep unclaimed FIFO items queued for the next turn before durable work.
    if !remainder_ids.is_empty() {
        restore_front_pending_messages(active_sessions, session_id, &remainder_ids).await;
    }

    if claim_ids.len() == 1 {
        return claim_single_pending_message(
            active_sessions,
            app_handle,
            session_id,
            claim_ids[0].clone(),
        )
        .await;
    }

    let fetched_messages = match load_messages_by_ids(claim_ids.clone()).await {
        Ok(msgs) => msgs,
        Err(e) => {
            restore_front_pending_messages(active_sessions, session_id, &claim_ids).await;
            return Err(e);
        }
    };

    let found_ids: HashSet<String> = fetched_messages.iter().map(|m| m.id.clone()).collect();
    for id in &claim_ids {
        if !found_ids.contains(id) {
            log::warn!(
                "Pending message {id} missing from DB for session {session_id}; dropping index"
            );
            if let Err(e) = get_pending_queue_repository().remove(id).await {
                log::warn!("Failed to drop orphan pending index for {id}: {e}");
            }
        }
    }

    if fetched_messages.is_empty() {
        emit_pending_queue_updated(active_sessions, app_handle, session_id).await?;
        return Ok(Vec::new());
    }

    if fetched_messages.len() == 1 {
        return claim_single_pending_message(
            active_sessions,
            app_handle,
            session_id,
            fetched_messages[0].id.clone(),
        )
        .await;
    }

    let mut merged_message = fetched_messages[0].clone();
    merged_message.content = merge_user_message_contents(&fetched_messages);
    merged_message.attachments = merge_user_message_attachments(&fetched_messages);
    merged_message.updated_at = chrono::Utc::now().timestamp_millis();

    let absorbed_ids: Vec<String> = fetched_messages
        .iter()
        .skip(1)
        .map(|m| m.id.clone())
        .collect();

    if let Err(e) = get_pending_queue_repository()
        .commit_merged_claim(&merged_message, &absorbed_ids)
        .await
    {
        restore_front_pending_messages(active_sessions, session_id, &claim_ids).await;
        return Err(e.to_string());
    }

    push_message_to_session_cache(active_sessions, session_id, &merged_message).await;
    emit_message_added(app_handle, session_id, &merged_message).await?;
    emit_pending_queue_updated(active_sessions, app_handle, session_id).await?;

    Ok(vec![merged_message])
}

async fn claim_single_pending_message(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: &str,
    message_id: String,
) -> Result<Vec<Message>, String> {
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
        return Ok(Vec::new());
    };

    emit_message_added(app_handle, session_id, &message).await?;
    emit_pending_queue_updated(active_sessions, app_handle, session_id).await?;
    Ok(vec![*message])
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
