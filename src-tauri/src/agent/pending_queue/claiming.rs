use crate::agent::state::AgentSession;
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use crate::repositories::pending_queue_repository::{PendingQueueEntry, PendingQueueRepository};
use crate::state::{get_message_repository, get_pending_queue_repository};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

use super::operations::{
    emit_message_added, emit_pending_queue_updated, push_message_to_session_cache,
};
use super::restore::{
    load_messages_by_ids, restore_front_pending_message, restore_front_pending_messages,
    restore_index_entry,
};

/// Max waiting prompts claimed and merged into one user message per LLM turn.
/// Remaining FIFO items stay queued for the next turn.
pub const MAX_PENDING_CLAIM_BATCH: usize = 8;

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

    let mut promoted_messages = Vec::new();
    // IDs not yet durably promoted — restore only this set on partial failure.
    let mut unpromoted_ids: Vec<String> = fetched_messages.iter().map(|m| m.id.clone()).collect();
    let mut i = 0;
    while i < fetched_messages.len() {
        if fetched_messages[i].role == "user" {
            let mut j = i + 1;
            while j < fetched_messages.len() && fetched_messages[j].role == "user" {
                j += 1;
            }
            let user_slice = &fetched_messages[i..j];
            if user_slice.len() == 1 {
                let promoted_id = user_slice[0].id.clone();
                let res = match claim_single_pending_message(
                    active_sessions,
                    app_handle,
                    session_id,
                    promoted_id.clone(),
                )
                .await
                {
                    Ok(res) => res,
                    Err(e) => {
                        // claim_single restores the failed id; restore only the tail.
                        unpromoted_ids.retain(|id| id != &promoted_id);
                        restore_front_pending_messages(
                            active_sessions,
                            session_id,
                            &unpromoted_ids,
                        )
                        .await;
                        return Err(e);
                    }
                };
                unpromoted_ids.retain(|id| id != &promoted_id);
                promoted_messages.extend(res);
            } else {
                let merged_contents =
                    crate::agent::message_merge::merge_user_message_contents(user_slice);
                let merged_attachments =
                    crate::agent::message_merge::merge_user_message_attachments(user_slice);

                let mut keeper = user_slice[0].clone();
                keeper.content = merged_contents;
                keeper.attachments = merged_attachments;
                keeper.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(keeper.created_at);

                let absorbed_ids: Vec<String> =
                    user_slice.iter().skip(1).map(|m| m.id.clone()).collect();
                let slice_ids: Vec<String> = user_slice.iter().map(|m| m.id.clone()).collect();

                if let Err(e) = get_pending_queue_repository()
                    .commit_merged_claim(&keeper, &absorbed_ids)
                    .await
                {
                    restore_front_pending_messages(active_sessions, session_id, &unpromoted_ids)
                        .await;
                    return Err(e.to_string());
                }

                for id in &slice_ids {
                    unpromoted_ids.retain(|pending_id| pending_id != id);
                }

                push_message_to_session_cache(active_sessions, session_id, &keeper).await;
                emit_message_added(app_handle, session_id, &keeper).await?;
                promoted_messages.push(keeper);
            }
            i = j;
        } else {
            // Non-user message (e.g. tool or assistant): promote individually without merge
            log::warn!(
                "claim_all_pending_messages: promoting non-user message {} (role: {}) individually without merge",
                fetched_messages[i].id,
                fetched_messages[i].role
            );
            let promoted_id = fetched_messages[i].id.clone();
            let res = match claim_single_pending_message(
                active_sessions,
                app_handle,
                session_id,
                promoted_id.clone(),
            )
            .await
            {
                Ok(res) => res,
                Err(e) => {
                    unpromoted_ids.retain(|id| id != &promoted_id);
                    restore_front_pending_messages(active_sessions, session_id, &unpromoted_ids)
                        .await;
                    return Err(e);
                }
            };
            unpromoted_ids.retain(|id| id != &promoted_id);
            promoted_messages.extend(res);
            i += 1;
        }
    }

    emit_pending_queue_updated(active_sessions, app_handle, session_id).await?;
    Ok(promoted_messages)
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
