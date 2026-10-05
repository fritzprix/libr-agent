/// Message search service layer.
///
/// Contains high-level business logic for search operations, orchestrating
/// between repositories, index storage, and the search engine itself.
use crate::repositories::MessageRepository;
use crate::search::index_storage::{get_index_path, write_index_atomic, IndexData, IndexMetadata};
use crate::search::message_index::{MessageDocument, MessageSearchEngine};
use crate::state::get_message_repository;

/// Rebuilds the search index for a specific session and persists it to disk.
///
/// This function:
/// 1. Fetches recent messages from the database
/// 2. Builds a new BM25 index
/// 3. Serializes and writes the index to disk
/// 4. Updates the index metadata in the database
///
/// # Arguments
/// * `session_id` - The session ID to rebuild the index for
///
/// # Returns
/// The newly built `MessageSearchEngine` on success
pub async fn rebuild_and_persist_index(session_id: &str) -> Result<MessageSearchEngine, String> {
    let repo = get_message_repository();
    let index_path = get_index_path(session_id)?;
    let max_docs = MessageSearchEngine::max_docs_from_env();

    let start_time = std::time::Instant::now();

    // Index columns only. Thinking blocks and attachments stay out of this read.
    let documents = repo
        .get_index_documents_by_session(session_id, max_docs as u64)
        .await
        .map_err(|e| format!("Failed to fetch messages for indexing: {e}"))?
        .into_iter()
        .map(MessageDocument::from)
        .collect();

    let engine =
        MessageSearchEngine::build_from_documents(session_id.to_string(), documents, max_docs)?;

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
    repo.update_index_meta(
        session_id,
        &index_path.to_string_lossy(),
        engine.doc_count(),
        rebuild_duration,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(engine)
}

/// Builds a temporary global search index from messages across all sessions.
///
/// This is used for global search when no session ID is specified.
/// The index is in-memory only and is not persisted to disk.
///
/// # Returns
/// A `MessageSearchEngine` containing recent messages from all sessions.
pub async fn build_global_temporary_index() -> Result<MessageSearchEngine, String> {
    let repo = get_message_repository();
    let max_docs = MessageSearchEngine::max_docs_from_env();

    let documents = repo
        .get_recent_index_documents(max_docs as u64)
        .await
        .map_err(|e| format!("Failed to fetch messages for global indexing: {e}"))?
        .into_iter()
        .map(MessageDocument::from)
        .collect();

    MessageSearchEngine::build_from_documents("global".to_string(), documents, max_docs)
}
