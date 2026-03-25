/// Attachments management commands
///
/// This module contains commands for managing attachments data, including
/// deletion of session-specific attachments and search indices.
use crate::services::AttachmentsService;

/// Delete attachments data for a session.
///
/// Removes SQLite rows (stores/contents/chunks) when a SQLite DB URL is configured,
/// and removes the attachments search index directory under the session workspace.
///
/// # Arguments
/// * `session_id` - The unique identifier of the session whose attachments should be deleted
///
/// # Returns
/// * `Ok(())` - Successfully deleted attachments data
/// * `Err(String)` - Error message if deletion fails
#[tauri::command]
pub async fn delete_attachments(session_id: String) -> Result<(), String> {
    AttachmentsService::delete_attachments(&session_id).await
}
