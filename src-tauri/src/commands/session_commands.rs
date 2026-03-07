use crate::services::SessionCleanupService;
use log::info;
use serde::{Deserialize, Serialize};
use tauri::command;

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

    SessionCleanupService::remove_session_complete(&session_id).await?;

    info!("✅ Fully removed session: {session_id}");

    Ok(SessionResponse {
        success: true,
        message: format!("Removed session: {session_id}"),
        session_id: Some(session_id),
        data: None,
    })
}
