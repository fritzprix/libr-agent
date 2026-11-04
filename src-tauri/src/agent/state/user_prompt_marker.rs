//! Persisted record of the tool call that waits on a user prompt (#2557).
//!
//! The pending prompt lives in memory, so an app restart loses it. Recovery
//! reads this record to close that tool call with the restart message instead
//! of the generic crash text. Only the tool call id is stored: never the
//! question payload, a secret value, or an answer.

use crate::repositories::SettingsRepository;
use std::future::Future;

const KEY_PREFIX: &str = "userPromptWaitingToolCall:";

tokio::task_local! {
    static CURRENT_TOOL_CALL_ID: String;
}

/// Run one tool call with its id visible to [`current_tool_call_id`].
///
/// A task-local does not cross `tokio::spawn`: a tool that asks the user from
/// a spawned task records nothing, and recovery falls back to the crash text.
pub async fn with_tool_call_id<F: Future>(tool_call_id: String, future: F) -> F::Output {
    CURRENT_TOOL_CALL_ID.scope(tool_call_id, future).await
}

/// The id of the tool call the current task runs, if the agent executor set one.
fn current_tool_call_id() -> Option<String> {
    CURRENT_TOOL_CALL_ID.try_with(Clone::clone).ok()
}

fn marker_key(session_id: &str) -> String {
    format!("{KEY_PREFIX}{session_id}")
}

/// Record that `tool_call_id` waits on a user prompt in `session_id`.
pub async fn persist_waiting_tool_call(
    repo: &dyn SettingsRepository,
    session_id: &str,
    tool_call_id: &str,
) {
    let value = serde_json::Value::String(tool_call_id.to_string());
    if let Err(error) = repo.set(&marker_key(session_id), value).await {
        log::warn!(
            "Failed to record the tool call waiting on a user prompt for session {}: {}",
            session_id,
            error
        );
    }
}

/// Remove the record once the prompt resolves.
async fn clear_waiting_tool_call(repo: &dyn SettingsRepository, session_id: &str) {
    if let Err(error) = repo.delete(&marker_key(session_id)).await {
        log::warn!(
            "Failed to clear the tool call waiting on a user prompt for session {}: {}",
            session_id,
            error
        );
    }
}

/// The record for one waiting prompt: written on `open`, removed on `close`.
///
/// Holds nothing when no repository is available or the prompt was not asked
/// from an agent tool call, so `close` has nothing to remove.
pub struct WaitingToolCallRecord<'a> {
    written: Option<(&'a dyn SettingsRepository, String)>,
}

impl<'a> WaitingToolCallRecord<'a> {
    pub async fn open(repo: Option<&'a dyn SettingsRepository>, session_id: &str) -> Self {
        let written = match (repo, current_tool_call_id()) {
            (Some(repo), Some(tool_call_id)) => {
                persist_waiting_tool_call(repo, session_id, &tool_call_id).await;
                Some((repo, session_id.to_string()))
            }
            _ => None,
        };
        Self { written }
    }

    pub async fn close(self) {
        if let Some((repo, session_id)) = self.written {
            clear_waiting_tool_call(repo, &session_id).await;
        }
    }
}

/// Read and remove the record. Used once, by recovery after a restart.
pub async fn take_waiting_tool_call(
    repo: &dyn SettingsRepository,
    session_id: &str,
) -> Option<String> {
    let stored = match repo.get(&marker_key(session_id)).await {
        Ok(stored) => stored?,
        Err(error) => {
            log::warn!(
                "Failed to read the tool call waiting on a user prompt for session {}: {}",
                session_id,
                error
            );
            return None;
        }
    };
    clear_waiting_tool_call(repo, session_id).await;
    serde_json::from_str::<String>(&stored.value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::settings;
    use crate::repositories::SqliteSettingsRepository;
    use sea_orm::{ConnectionTrait, Database, Schema};

    async fn repo() -> SqliteSettingsRepository {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        let schema = Schema::new(db.get_database_backend());
        let stmt = schema.create_table_from_entity(settings::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("settings table");
        SqliteSettingsRepository::new(db)
    }

    #[tokio::test]
    async fn take_returns_the_recorded_id_once() {
        let repo = repo().await;
        persist_waiting_tool_call(&repo, "s1", "call-1").await;

        assert_eq!(
            take_waiting_tool_call(&repo, "s1").await.as_deref(),
            Some("call-1")
        );
        assert_eq!(take_waiting_tool_call(&repo, "s1").await, None);
    }

    #[tokio::test]
    async fn clear_leaves_no_record() {
        let repo = repo().await;
        persist_waiting_tool_call(&repo, "s1", "call-1").await;
        clear_waiting_tool_call(&repo, "s1").await;

        assert!(repo.list().await.expect("list").is_empty());
    }

    #[tokio::test]
    async fn current_tool_call_id_is_set_only_inside_the_scope() {
        assert_eq!(current_tool_call_id(), None);
        let inside =
            with_tool_call_id("call-1".to_string(), async { current_tool_call_id() }).await;
        assert_eq!(inside.as_deref(), Some("call-1"));
        assert_eq!(current_tool_call_id(), None);
    }

    #[tokio::test]
    async fn a_resolved_prompt_leaves_no_record() {
        let repo = repo().await;
        with_tool_call_id("call-1".to_string(), async {
            let record = WaitingToolCallRecord::open(Some(&repo), "s1").await;
            let stored = repo.list().await.expect("list");
            assert_eq!(stored.len(), 1);
            assert_eq!(stored[0].value, "\"call-1\"");
            record.close().await;
        })
        .await;

        assert!(repo.list().await.expect("list").is_empty());
    }

    #[tokio::test]
    async fn a_prompt_outside_a_tool_call_records_nothing() {
        let repo = repo().await;
        let record = WaitingToolCallRecord::open(Some(&repo), "s1").await;

        assert!(repo.list().await.expect("list").is_empty());
        record.close().await;
    }

    #[tokio::test]
    async fn records_are_per_session() {
        let repo = repo().await;
        persist_waiting_tool_call(&repo, "s1", "call-1").await;
        persist_waiting_tool_call(&repo, "s2", "call-2").await;

        assert_eq!(
            take_waiting_tool_call(&repo, "s2").await.as_deref(),
            Some("call-2")
        );
        assert_eq!(
            take_waiting_tool_call(&repo, "s1").await.as_deref(),
            Some("call-1")
        );
    }
}
