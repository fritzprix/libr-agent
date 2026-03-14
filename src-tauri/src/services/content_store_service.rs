use crate::repositories::ContentStoreRepository;
use crate::session::get_session_manager;
use crate::state::{get_content_store_repository, get_sqlite_db_url};
use tokio::fs as tokio_fs;

pub struct ContentStoreService;

impl ContentStoreService {
    /// Delete content store data for a session.
    ///
    /// Removes `SQLite` rows (stores/contents/chunks) when a `SQLite` DB URL is configured,
    /// and removes the content store search index directory under the session workspace.
    pub async fn delete_content_store(session_id: &str) -> Result<(), String> {
        // 1) Remove SQLite entries if SQLITE_DB_URL configured
        if get_sqlite_db_url().is_some() {
            let repo = get_content_store_repository();
            repo.delete_by_session(session_id)
                .await
                .map_err(|e| e.to_string())?;
        }

        // 2) Remove content_store_search index directory in session workspace
        let session_manager =
            get_session_manager().map_err(|e| format!("Session manager error: {e}"))?;
        let workspace_dir = session_manager.get_session_workspace_dir_by_id(session_id);
        let search_index_dir = workspace_dir.join("content_store_search");

        if search_index_dir.exists() {
            tokio_fs::remove_dir_all(&search_index_dir)
                .await
                .map_err(|e| format!("Failed to remove search index directory: {e}"))?;
        }

        Ok(())
    }
}
