use crate::session::get_session_manager;
use log::info;
use serde::{Deserialize, Serialize};
use tauri::command;
use crate::services::SessionCleanupService;

// ============================================================================
// Session Manager Commands
// Used for workspace-level session isolation (file system separation)
// Agent V2 sessions are managed via agent_commands.rs (SQLite-based)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionResponse {
    pub success: bool,
    pub message: String,
    pub session_id: Option<String>,
    pub data: Option<serde_json::Value>,
}

/// Remove a specific session
/// Deletes search index, metadata, workspace directory, and all associated resources
#[command]
pub async fn remove_session(session_id: String) -> Result<SessionResponse, String> {
    info!("🗑️  Removing session via services: {session_id}");

    // 1. Clean up auxiliary resources (Index, DB) via SessionCleanupService
    // This handles the "side effects" and external data.
    let message_repo = crate::state::get_message_repository();

    // Pass the message repository reference directly, trusting it implements MessageRepository trait
    SessionCleanupService::cleanup_auxiliary_resources(
        &session_id,
        message_repo // was message_repo.as_ref() which caused error
    ).await.map_err(|e| format!("Failed to cleanup auxiliary resources: {e}"))?;

    // 2. Remove workspace directory and update internal session pool via SessionManager
    // This handles the core "session existence" and filesystem.
    let session_manager = get_session_manager().map_err(|e| format!("Failed to get session manager: {e}"))?;
    session_manager
        .remove_session(&session_id)
        .await
        .map_err(|e| format!("Failed to remove session workspace: {e}"))?;

    info!("✅ Fully removed session: {session_id}");

    Ok(SessionResponse {
        success: true,
        message: format!("Removed session: {session_id}"),
        session_id: Some(session_id),
        data: None,
    })
}
