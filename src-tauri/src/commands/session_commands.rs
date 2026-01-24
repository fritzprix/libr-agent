use crate::session::get_session_manager;
use log::{error, info};
use serde::{Deserialize, Serialize};
use tauri::command;

// ============================================================================
// Session Manager Commands
// Used for workspace-level session isolation (file system separation)
// Agent V2 sessions are managed via agent_commands.rs (SQLite-based)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionSwitchRequest {
    pub session_id: String,
    pub use_async: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionResponse {
    pub success: bool,
    pub message: String,
    pub session_id: Option<String>,
    pub data: Option<serde_json::Value>,
}

/// Switch to a specific session for workspace isolation
/// Used by BuiltInToolProvider to switch backend workspace when changing Agent V2 sessions
/// Switch to a specific session (Legacy/Deprecated)
/// In Agent V2, sessions are isolated by ID and do not rely on global context switching.
/// This command is preserved as a no-op to prevent frontend errors during transition.
#[command]
pub async fn switch_session(request: SessionSwitchRequest) -> Result<SessionResponse, String> {
    log::warn!("Call to deprecated switch_session for '{}'. Global session switching is disabled in Agent V2.", request.session_id);

    Ok(SessionResponse {
        success: true,
        message: format!(
            "Session switch ignored (Agent V2 isolation enabled): {}",
            request.session_id
        ),
        session_id: Some(request.session_id),
        data: None,
    })
}

/// Remove a specific session
/// Deletes search index, metadata, workspace directory, and all associated resources
#[command]
pub async fn remove_session(session_id: String) -> Result<SessionResponse, String> {
    use crate::repositories::MessageRepository;
    use crate::search::index_storage::delete_index;

    info!("🗑️  Removing session: {session_id}");

    // Step 1: Delete BM25 search index file and metadata
    if let Err(e) = delete_index(&session_id) {
        error!("Failed to delete search index for session {session_id}: {e}");
        // Continue with removal even if index deletion fails (best-effort)
    }

    // Step 2: Delete index metadata from database
    let repo = crate::state::get_message_repository();
    if let Err(e) = repo.delete_index_metadata(&session_id).await {
        error!("Failed to delete index metadata for session {session_id}: {e}");
        // Continue with removal even if metadata deletion fails (best-effort)
    } else {
        info!("✅ Deleted index metadata for session: {session_id}");
    }

    // Step 3: Remove session workspace directory and other resources
    let session_manager =
        get_session_manager().map_err(|e| format!("Failed to get session manager: {e}"))?;

    session_manager
        .remove_session(&session_id)
        .await
        .map_err(|e| format!("Failed to remove session: {e}"))?;

    info!("✅ Removed session: {session_id}");

    Ok(SessionResponse {
        success: true,
        message: format!("Removed session: {session_id}"),
        session_id: Some(session_id),
        data: None,
    })
}
