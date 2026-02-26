use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionMetadata;
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
