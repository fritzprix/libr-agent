/// Content store management commands
///
/// This module contains commands for managing content store data, including
/// deletion of session-specific content stores and search indices.
use crate::session::get_session_manager;
use crate::state::get_sqlite_db_url;
use sqlx::sqlite::SqlitePool;
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
    if let Some(db_url) = get_sqlite_db_url() {
        // Mirror storage.rs handling for sqlite path (strip sqlite:// prefix if present)
        let db_path = if let Some(p) = db_url.strip_prefix("sqlite://") {
            p.to_string()
        } else {
            db_url.clone()
        };

        // Connect and execute deletion in a transaction
        let pool = SqlitePool::connect(&db_path)
            .await
            .map_err(|e| format!("Failed to connect to SQLite DB: {e}"))?;

        // Execute deletion queries directly on the pool (no explicit transaction)
        sqlx::query(
            "DELETE FROM chunks WHERE content_id IN (SELECT id FROM contents WHERE session_id = ?)",
        )
        .bind(&session_id)
        .execute(&pool)
        .await
        .map_err(|e| format!("Failed to delete chunks: {e}"))?;

        sqlx::query("DELETE FROM contents WHERE session_id = ?")
            .bind(&session_id)
            .execute(&pool)
            .await
            .map_err(|e| format!("Failed to delete contents: {e}"))?;

        sqlx::query("DELETE FROM stores WHERE session_id = ?")
            .bind(&session_id)
            .execute(&pool)
            .await
            .map_err(|e| format!("Failed to delete store: {e}"))?;
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
