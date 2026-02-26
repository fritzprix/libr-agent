use crate::agent::session_manager::AgentSessionManager;
use crate::repositories::MessageRepository;
use crate::search::message_index::{MessageSearchEngine, SearchResult};
use crate::state::get_message_repository;
use crate::utils::pagination::{paginate_in_memory, Page};
use std::collections::HashMap;
use std::sync::Mutex;

/// Global cache for loaded search indices (session_id -> MessageSearchEngine)
static INDEX_CACHE: once_cell::sync::Lazy<Mutex<HashMap<String, MessageSearchEngine>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

pub struct MessageService;

impl MessageService {
    /// Delete a single message by ID.
    /// Also removes the message from the in-memory cache of active sessions to maintain consistency.
    pub async fn delete_message(
        message_id: String,
        session_manager: &AgentSessionManager,
    ) -> Result<(), String> {
        // First, get the message to find which session it belongs to
        let repo = get_message_repository();

        // Find the message's session_id before deleting
        // We need to load it to identify which session cache to update
        let message = repo
            .get_by_id(&message_id)
            .await
            .map_err(|e| format!("Failed to query message by id {}: {}", message_id, e))?;

        if message.is_none() {
            log::warn!(
                "delete_message: message {} not found in database, nothing to delete",
                message_id
            );
        }

        let session_id = message.map(|m| m.session_id);

        // Delete from database
        repo.delete_by_id(&message_id)
            .await
            .map_err(|e| e.to_string())?;

        // Update in-memory cache if this session is active
        if let Some(sid) = session_id {
            if let Err(e) = session_manager
                .remove_message_from_cache(&sid, &message_id)
                .await
            {
                log::warn!(
                    "Failed to remove message {} from in-memory cache for session {}: {}",
                    message_id,
                    sid,
                    e
                );
                // Don't fail the entire operation - DB is already updated
            }
        }

        Ok(())
    }

    /// Load or rebuild the search index for a session.
    async fn get_or_build_index(session_id: &str) -> Result<MessageSearchEngine, String> {
        let repo = get_message_repository();

        // Check if index exists and is up to date
        let is_dirty = repo
            .is_index_dirty(session_id)
            .await
            .map_err(|e| e.to_string())?;

        // Try to load from cache first
        {
            let cache = INDEX_CACHE
                .lock()
                .map_err(|e| format!("Cache lock error: {e}"))?;
            if let Some(engine) = cache.get(session_id) {
                if !is_dirty {
                    return Ok(engine.clone());
                }
            }
        }

        // If dirty or not cached, rebuild
        let engine = crate::search::service::rebuild_and_persist_index(session_id).await?;

        // Cache the engine
        {
            let mut cache = INDEX_CACHE
                .lock()
                .map_err(|e| format!("Cache lock error: {e}"))?;
            cache.insert(session_id.to_string(), engine.clone());
        }

        Ok(engine)
    }

    /// Search messages using BM25 full-text search.
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `session_id` - Optional session ID to search within (if None, searches all sessions)
    /// * `page` - Page number (1-indexed)
    /// * `page_size` - Number of results per page
    ///
    /// # Returns
    /// Paginated search results with relevance scores
    pub async fn search_messages(
        query: String,
        session_id: Option<String>,
        page: u64,
        page_size: u64,
    ) -> Result<Page<SearchResult>, String> {
        if query.trim().is_empty() {
            return Ok(Page::new(Vec::new(), page, page_size, 0));
        }

        // Calculate search limit to ensure we fetch enough results for the requested page
        // We multiply by 2 to account for potential relevance variance or future filtering
        let search_limit = page
            .saturating_mul(page_size)
            .saturating_mul(2)
            .min(usize::MAX as u64) as usize;

        let all_results = if let Some(target_session) = session_id {
            // Per-session behavior (cached index)
            let engine = Self::get_or_build_index(&target_session).await?;
            engine.search(&query, search_limit)?
        } else {
            // Global search: build a temporary index from messages across all sessions
            let engine = crate::search::service::build_global_temporary_index().await?;
            engine.search(&query, search_limit)?
        };

        // Use shared in-memory pagination logic
        Ok(paginate_in_memory(all_results, page, page_size))
    }
}
