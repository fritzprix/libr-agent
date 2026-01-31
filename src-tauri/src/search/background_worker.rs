/// Background indexing worker for message search.
///
/// Periodically checks for dirty sessions (those with messages newer than
/// the last index build) and rebuilds their search indices in the background.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use crate::repositories::MessageRepository;
use crate::state::get_message_repository;

/// Background worker that periodically reindexes dirty sessions.
#[allow(dead_code)]
pub struct IndexingWorker {
    /// Flag to signal worker shutdown
    shutdown: Arc<AtomicBool>,
    /// Worker task handle
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl IndexingWorker {
    /// Creates a new indexing worker and starts it.
    ///
    /// # Arguments
    /// * `check_interval` - Duration between checks for dirty sessions
    pub fn new(check_interval: Duration) -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        let task_handle = tokio::spawn(async move {
            worker_loop(shutdown_clone, check_interval).await;
        });

        Self {
            shutdown,
            task_handle: Some(task_handle),
        }
    }

    /// Stops the background worker gracefully.
    #[allow(dead_code)]
    pub async fn stop(mut self) {
        self.shutdown.store(true, Ordering::Relaxed);

        if let Some(handle) = self.task_handle.take() {
            let _ = handle.await;
        }
    }
}

/// Main worker loop that checks for and reindexes dirty sessions.
async fn worker_loop(shutdown: Arc<AtomicBool>, check_interval: Duration) {
    log::info!("🔄 Message search indexing worker started");

    while !shutdown.load(Ordering::Relaxed) {
        // Sleep first to avoid immediate check on startup
        sleep(check_interval).await;

        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        // Find and reindex dirty sessions
        if let Err(e) = reindex_dirty_sessions().await {
            log::error!("❌ Background reindexing failed: {e}");
        }
    }

    log::info!("✅ Message search indexing worker stopped");
}

/// Finds all sessions with dirty indices and rebuilds them.
async fn reindex_dirty_sessions() -> Result<(), String> {
    // Get all unique session IDs
    let sessions = crate::get_message_repository()
        .get_distinct_sessions()
        .await
        .map_err(|e| format!("Failed to fetch session IDs: {e}"))?;

    for session_id in sessions {
        // Check if index is dirty
        let repo = get_message_repository();
        let is_dirty = repo
            .is_index_dirty(&session_id)
            .await
            .map_err(|e| e.to_string())?;

        if is_dirty {
            log::info!("🔨 Rebuilding index for session: {session_id}");

            if let Err(e) = rebuild_session_index(&session_id).await {
                log::error!("❌ Failed to rebuild index for session {session_id}: {e}");
                continue;
            }

            log::info!("✅ Index rebuilt for session: {session_id}");
        }
    }

    Ok(())
}

/// Rebuilds the search index for a specific session.
async fn rebuild_session_index(session_id: &str) -> Result<(), String> {
    crate::search::service::rebuild_and_persist_index(session_id)
        .await
        .map(|_| ())
}
