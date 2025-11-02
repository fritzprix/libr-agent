use crate::repositories::ContentStoreRepository;
/// Content store management commands
///
/// This module contains commands for managing content store data, including
/// deletion of session-specific content stores and search indices.
use crate::session::get_session_manager;
use crate::state::{get_content_store_repository, get_sqlite_db_url};
use tokio::fs as tokio_fs;

/// Delete content store data for a session.
///
/// Removes SQLite rows (stores/contents/chunks) when a SQLite DB URL is configured,
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
    // 1) Remove SQLite entries if SQLITE_DB_URL configured
    if get_sqlite_db_url().is_some() {
        let repo = get_content_store_repository();
        repo.delete_by_session(&session_id)
            .await
            .map_err(|e| e.to_string())?;
    }

    // 2) Remove content_store_search index directory in session workspace
    let session_manager =
        get_session_manager().map_err(|e| format!("Session manager error: {e}"))?;
    let workspace_dir = session_manager.get_session_workspace_dir();
    let search_index_dir = workspace_dir.join("content_store_search");

    if search_index_dir.exists() {
        tokio_fs::remove_dir_all(&search_index_dir)
            .await
            .map_err(|e| format!("Failed to remove search index directory: {e}"))?;
    }

    Ok(())
}
