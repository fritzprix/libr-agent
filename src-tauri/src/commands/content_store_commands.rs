/// Content store management commands
///
/// This module contains commands for managing content store data, including
/// deletion of session-specific content stores and search indices.
use crate::services::ContentStoreService;

/// Delete content store data for a session.
///
/// Removes `SQLite` rows (stores/contents/chunks) when a `SQLite` DB URL is configured,
/// and removes the content store search index directory under the session workspace.
///
/// # Arguments
/// * `session_id` - The unique identifier of the session whose content store should be deleted
///
/// # Returns
/// * `Ok(())` - Successfully deleted content store data
/// * `Err(String)` - Error message if deletion fails
#[tauri::command]
pub async fn delete_content_store(session_id: String) -> Result<(), String> {
    ContentStoreService::delete_content_store(&session_id).await
}
