//! Persistence of the terminal assistant message and of the prompt-token
//! checkpoint, both used by the response handler.

use crate::agent::state::{enforce_cached_message_limit, AgentSession};
use crate::models::chat::Message;
use crate::repositories::message_repository::MessageRepository as MessageRepositoryTrait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(super) fn assistant_message_has_only_ui_tool_calls(message: &Message) -> bool {
    message
        .tool_calls
        .as_ref()
        .map(|tool_calls| {
            !tool_calls.is_empty()
                && tool_calls
                    .iter()
                    .all(|tool_call| tool_call.function.name.starts_with("ui__"))
        })
        .unwrap_or(false)
}

pub(super) async fn persist_assistant_message_to_db(message: &Message) -> Result<(), String> {
    let repo = crate::state::get_message_repository();
    repo.insert(message).await.map_err(|error| {
        log::error!(
            "Failed to save assistant message to DB: msg_id={}, error={}",
            message.id,
            error
        );
        format!("Failed to save assistant message to DB: {}", error)
    })
}

pub(super) async fn cache_assistant_message(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    assistant_message: &Message,
) {
    let sessions = active_sessions.read().await;
    if let Some(session) = sessions.get(session_id) {
        let mut messages = session.messages.write().await;
        messages.push(assistant_message.clone());
        enforce_cached_message_limit(&mut messages);

        log::info!(
            "🤖 Message stack after assistant message: session={}, count={}, latest_message={}",
            session_id,
            messages.len(),
            assistant_message.id
        );
    }
}

pub(super) fn extract_prompt_tokens(message: &Message) -> Option<i64> {
    message
        .usage
        .as_ref()
        .and_then(|usage| usage.get("promptTokens"))
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|v| i64::try_from(v).ok()))
        })
}

pub(super) async fn persist_prompt_token_checkpoint(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    prompt_tokens: i64,
) {
    let (checkpoint_handle, message_handle) = {
        let sessions = active_sessions.read().await;
        let Some(session) = sessions.get(session_id) else {
            return;
        };
        (
            session.last_submitted_input_message_id.clone(),
            session.messages.clone(),
        )
    };

    let checkpoint_id = checkpoint_handle.read().await.clone();
    let Some(checkpoint_id) = checkpoint_id else {
        return;
    };

    let checkpoint_message = {
        let mut messages = message_handle.write().await;
        let Some(message) = messages
            .iter_mut()
            .find(|message| message.id == checkpoint_id)
        else {
            log::warn!(
                "Failed to stamp prompt-token checkpoint: session={}, checkpoint_id={} missing from cache",
                session_id,
                checkpoint_id
            );
            return;
        };

        message.prompt_tokens = Some(prompt_tokens);
        message.clone()
    };

    let repo = crate::state::get_message_repository();
    if let Err(error) = repo.insert(&checkpoint_message).await {
        log::error!(
            "Failed to persist prompt-token checkpoint: session={}, checkpoint_id={}, error={}",
            session_id,
            checkpoint_message.id,
            error
        );
    }
}
