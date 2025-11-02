use sqlx::Row;
/// Background indexing worker for message search.
///
/// Periodically checks for dirty sessions (those with messages newer than
/// the last index build) and rebuilds their search indices in the background.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use crate::repositories::MessageRepository;
use crate::search::index_storage::{get_index_path, write_index_atomic, IndexData, IndexMetadata};
use crate::search::message_index::{MessageDocument, MessageSearchEngine};
use crate::state::{get_message_repository, get_sqlite_pool};

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
    let pool = get_sqlite_pool();

    // Get all unique session IDs
    let sessions = sqlx::query("SELECT DISTINCT session_id FROM messages")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch session IDs: {e}"))?;

    for row in sessions {
        let session_id: String = row
            .try_get("session_id")
            .map_err(|e| format!("Failed to get session_id: {e}"))?;

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
    let pool = get_sqlite_pool();
    let index_path = get_index_path(session_id)?;
    let max_docs = MessageSearchEngine::max_docs_from_env();

    let start_time = std::time::Instant::now();

    // Fetch messages from database (most recent max_docs)
    let messages = sqlx::query(
        r#"
        SELECT id, session_id, content, created_at
        FROM messages
        WHERE session_id = ?
        ORDER BY created_at DESC
        LIMIT ?
        "#,
    )
    .bind(session_id)
    .bind(max_docs as i64)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch messages for indexing: {e}"))?;

    // Convert to MessageDocument
    let documents: Vec<MessageDocument> = messages
        .into_iter()
        .map(|row| MessageDocument {
            id: row
                .try_get("id")
                .map_err(|e| format!("Failed to get id: {e}"))
                .unwrap(),
            session_id: row
                .try_get("session_id")
                .map_err(|e| format!("Failed to get session_id: {e}"))
                .unwrap(),
            content: row
                .try_get("content")
                .map_err(|e| format!("Failed to get content: {e}"))
                .unwrap(),
            created_at: row
                .try_get("created_at")
                .map_err(|e| format!("Failed to get created_at: {e}"))
                .unwrap(),
        })
        .collect();

    // Build index
    let mut engine = MessageSearchEngine::new(session_id.to_string(), max_docs);
    engine.add_documents(documents)?;

    // Persist to disk
    let serialized = engine.serialize()?;
    let index_data = IndexData {
        metadata: IndexMetadata {
            version: 1,
            session_id: session_id.to_string(),
            doc_count: engine.doc_count(),
            last_built_at: chrono::Utc::now().timestamp_millis(),
        },
        index_content: serialized,
    };

    write_index_atomic(&index_path, &index_data)?;

    // Update metadata in database
    let rebuild_duration = start_time.elapsed().as_millis() as i64;
    let repo = get_message_repository();
    repo.update_index_meta(
        session_id,
        &index_path.to_string_lossy(),
        engine.doc_count(),
        rebuild_duration,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}
