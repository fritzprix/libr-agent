use crate::repositories::session_repository::SessionRepository as SessionRepositoryTrait;
use crate::repositories::MessageRepository;
use crate::search::index_storage::delete_index;
use log::{error, info};

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
        message_repo: &impl MessageRepository,
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

    /// Recursively collect all descendant session IDs (children, grandchildren, etc.)
    pub async fn collect_descendant_ids(session_id: &str) -> Result<Vec<String>, String> {
        let session_repo = crate::state::get_session_repository();
        let mut all_descendants = Vec::new();
        let mut queue = vec![session_id.to_string()];

        while let Some(current_id) = queue.pop() {
            let children = session_repo
                .get_child_session_ids(&current_id)
                .await
                .map_err(|e| format!("Failed to get children for {}: {}", current_id, e))?;

            for child_id in children {
                all_descendants.push(child_id.clone());
                queue.push(child_id);
            }
        }

        Ok(all_descendants)
    }

    /// Delete workspace directory for a session
    pub async fn delete_session_workspace(session_id: &str) -> Result<(), String> {
        match crate::session::get_session_manager() {
            Ok(manager) => {
                // Ensure workspace is loaded into pool before attempting removal
                let _ = manager.get_session_workspace_dir_by_id(session_id);
                if let Err(e) = manager.remove_session(session_id).await {
                    log::warn!(
                        "Failed to remove workspace for session {}: {}",
                        session_id,
                        e
                    );
                }
            }
            Err(e) => {
                log::warn!("Failed to get session manager for workspace cleanup: {}", e);
            }
        }
        Ok(())
    }

    /// Delete an agent session data cascade
    ///
    /// **Cascade Philosophy:** "부모를 지우면 자식도 지워진다"
    /// - DB-level CASCADE automatically deletes child session records
    /// - Manually deletes workspace directories for all descendants before DB deletion
    pub async fn delete_session_data_cascade(
        session_id: &str,
        descendant_ids: &[String],
    ) -> Result<(), String> {
        for descendant_id in descendant_ids {
            Self::delete_session_workspace(descendant_id).await?;

            if let Err(e) = delete_index(descendant_id) {
                log::warn!(
                    "Failed to delete search index for descendant {}: {}",
                    descendant_id,
                    e
                );
            }
        }

        Self::delete_session_workspace(session_id).await?;

        if let Err(e) = delete_index(session_id) {
            log::warn!(
                "Failed to delete search index for session {}: {}",
                session_id,
                e
            );
        }

        let session_repo = crate::state::get_session_repository();
        session_repo
            .delete_session(session_id)
            .await
            .map_err(|e| format!("Failed to delete session metadata: {}", e))?;

        Ok(())
    }

    /// Delete only this session data, leaving children as orphaned top-level sessions.
    pub async fn delete_session_data_only(session_id: &str) -> Result<(), String> {
        Self::delete_session_workspace(session_id).await?;

        if let Err(e) = delete_index(session_id) {
            log::warn!(
                "Failed to delete search index for session {}: {}",
                session_id,
                e
            );
        }

        let session_repo = crate::state::get_session_repository();
        session_repo
            .orphan_and_delete_session(session_id)
            .await
            .map_err(|e| format!("Failed to delete session metadata: {}", e))?;

        Ok(())
    }
}
