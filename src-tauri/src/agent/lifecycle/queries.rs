use crate::repositories::session_repository::SessionRepository;
use crate::repositories::{SessionListCursor, SessionListPage, SessionMetadata};
use std::sync::Arc;

/// Get session metadata
pub async fn get_session(
    session_repo: &Arc<dyn SessionRepository>,
    session_id: &str,
) -> Result<Option<SessionMetadata>, String> {
    session_repo
        .get_session(session_id)
        .await
        .map_err(|e| format!("Failed to get session: {}", e))
}

/// Get all sessions from database
pub async fn get_all_sessions(
    session_repo: &Arc<dyn SessionRepository>,
) -> Result<Vec<SessionMetadata>, String> {
    session_repo
        .get_all_sessions()
        .await
        .map_err(|e| format!("Failed to get all sessions: {}", e))
}

/// List sessions using cursor pagination.
pub async fn list_sessions(
    session_repo: &Arc<dyn SessionRepository>,
    cursor: Option<SessionListCursor>,
    limit: u64,
) -> Result<SessionListPage, String> {
    session_repo
        .list_sessions(cursor, limit)
        .await
        .map_err(|e| format!("Failed to list sessions: {}", e))
}

/// List sessions with unread attention state.
pub async fn list_attention_sessions(
    session_repo: &Arc<dyn SessionRepository>,
) -> Result<Vec<SessionMetadata>, String> {
    session_repo
        .list_attention_sessions()
        .await
        .map_err(|e| format!("Failed to list attention sessions: {}", e))
}
