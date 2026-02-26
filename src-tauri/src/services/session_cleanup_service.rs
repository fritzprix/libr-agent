use log::{error, info};
use crate::repositories::MessageRepository;
use crate::search::index_storage::delete_index;

pub struct SessionCleanupService;

impl SessionCleanupService {
    /// Remove auxiliary resources for a session:
    /// 1. Search Index (Filesystem)
    /// 2. Database Metadata (SQL)
    ///
    /// Note: This does NOT remove the workspace directory or update the session pool.
    /// The caller must handle that via SessionManager or DirectoryService.
    pub async fn cleanup_auxiliary_resources(
        session_id: &str,
        message_repo: &impl MessageRepository
    ) -> Result<(), String> {
        info!("🗑️  Cleaning up auxiliary resources for session: {session_id}");

        // Step 1: Delete BM25 search index file
        if let Err(e) = delete_index(session_id) {
            error!("Failed to delete search index for session {session_id}: {e}");
            // Continue with removal even if index deletion fails (best-effort)
        } else {
             info!("✅ Deleted search index for session: {session_id}");
        }

        // Step 2: Delete index metadata from database
        if let Err(e) = message_repo.delete_index_metadata(session_id).await {
            error!("Failed to delete index metadata for session {session_id}: {e}");
            // Continue with removal even if metadata deletion fails (best-effort)
        } else {
            info!("✅ Deleted index metadata for session: {session_id}");
        }

        Ok(())
    }
}
