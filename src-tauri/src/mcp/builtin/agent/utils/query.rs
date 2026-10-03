use crate::agent::AgentSessionManager;
use crate::repositories::{MessageRepository, SessionMetadata, SessionRepository};
use serde_json::Value;

use super::output::{latest_session_output, session_output_is_missing};

pub const CHECK_SESSION_RESULT_MESSAGE_LIMIT: u64 = 20;

async fn fetch_cached_session_messages_newest_first(
    session_id: &str,
    limit: usize,
) -> Option<Vec<Value>> {
    let sessions = crate::state::try_get_active_sessions()?;
    let active = sessions.read().await;
    let session = active.get(session_id)?;
    let cached_messages = session.messages.read().await;
    if cached_messages.is_empty() {
        return None;
    }

    let take = limit.min(cached_messages.len());
    let newest_first = cached_messages
        .iter()
        .rev()
        .take(take)
        .filter_map(|message| serde_json::to_value(message).ok())
        .collect::<Vec<_>>();

    Some(newest_first)
}

pub(crate) fn select_preferred_session_messages(
    db_messages: Vec<Value>,
    cached_messages: Option<Vec<Value>>,
) -> Vec<Value> {
    let Some(cached) = cached_messages else {
        return db_messages;
    };

    if let Some(db_latest) = db_messages.first() {
        if let Some(db_id) = db_latest.get("id").and_then(|id| id.as_str()) {
            let cache_contains_db_latest = cached
                .iter()
                .any(|m| m.get("id").and_then(|id| id.as_str()) == Some(db_id));
            if !cache_contains_db_latest {
                return db_messages;
            }
        }
    }

    let db_output = latest_session_output(&db_messages);
    let cached_output = latest_session_output(&cached);

    if session_output_is_missing(&cached_output) {
        return db_messages;
    }

    if session_output_is_missing(&db_output) {
        return cached;
    }

    if cached.len() > db_messages.len() {
        return cached;
    }

    if cached_output != db_output && cached.len() >= db_messages.len() {
        return cached;
    }

    db_messages
}

/// Fetch newest-first messages for checkSession result assembly.
///
/// Requires [`StorageSessionId`] so `display_session_id(...)` cannot be passed
/// by accident. When the session repository is initialized, also rejects ids
/// that are not an exact `sessions.id` row (legacy display-token footgun).
pub async fn fetch_session_messages_for_result(
    storage_session_id: &crate::utils::session_id::StorageSessionId,
    limit: u64,
) -> Result<Vec<Value>, String> {
    let session_id = storage_session_id.as_str();

    if let Some(session_repo) = crate::state::try_get_session_repository() {
        let exists = session_repo
            .get_session(session_id)
            .await
            .map_err(|e| format!("Failed to verify session id for message lookup: {e}"))?
            .is_some();
        if !exists {
            return Err(format!(
                "BUG: session message lookup used unknown id '{session_id}'. \
                 Pass StorageSessionId::from_resolved(SessionMetadata.id), never \
                 display_session_id() — display tokens yield empty history and false \
                 \"No final answer yet.\""
            ));
        }
    }

    let repo = crate::state::get_message_repository();
    let messages = repo
        .get_messages_by_session(session_id, limit)
        .await
        .map_err(|e| format!("Failed to fetch session messages: {}", e))?;

    let db_messages: Vec<Value> = messages
        .into_iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<Value>, serde_json::Error>>()
        .map_err(|e| format!("Failed to serialize session messages: {}", e))?;

    let cached = fetch_cached_session_messages_newest_first(session_id, limit as usize).await;

    Ok(select_preferred_session_messages(db_messages, cached))
}

pub fn extract_assistant_id_from_session_value(session: &Value) -> Option<String> {
    session
        .get("assistantId")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn session_metadata_to_value(session: &SessionMetadata) -> Result<Value, String> {
    let mut value =
        serde_json::to_value(session).map_err(|e| format!("Failed to serialize session: {}", e))?;

    if let Some(assistant_id) = extract_assistant_id_from_session_value(&value) {
        if let Some(object) = value.as_object_mut() {
            object.insert("assistantId".to_string(), Value::String(assistant_id));
        }
    }

    Ok(value)
}

pub async fn fetch_session_value(
    manager: &AgentSessionManager,
    session_id: &str,
) -> Result<Option<Value>, String> {
    manager
        .get_session(session_id)
        .await?
        .map(|session| session_metadata_to_value(&session))
        .transpose()
}

pub async fn count_session_turns(session_id: &str) -> usize {
    let repo = crate::state::get_message_repository();
    let messages = repo
        .get_messages_by_session(session_id, 1000)
        .await
        .unwrap_or_default();
    messages.iter().filter(|m| m.role == "assistant").count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn assistant_json(id: &str, text: &str) -> Value {
        json!({
            "id": id,
            "role": "assistant",
            "content": [{"type": "text", "text": text}]
        })
    }

    #[test]
    fn select_preferred_prefers_cache_when_db_lags_behind_terminal_assistant() {
        let db_messages = vec![assistant_json("asst-1", "older answer")];
        let cached_messages = vec![
            assistant_json("asst-2", "final answer"),
            assistant_json("asst-1", "older answer"),
        ];

        let selected = select_preferred_session_messages(db_messages, Some(cached_messages));

        assert_eq!(latest_session_output(&selected), "final answer");
    }

    #[test]
    fn select_preferred_keeps_db_when_cache_is_stale() {
        let db_messages = vec![assistant_json("asst-2", "authoritative answer")];
        let cached_messages = vec![assistant_json("asst-1", "stale cache")];

        let selected = select_preferred_session_messages(db_messages, Some(cached_messages));

        assert_eq!(latest_session_output(&selected), "authoritative answer");
    }
}
