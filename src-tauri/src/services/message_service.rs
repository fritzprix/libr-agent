use crate::agent::session_manager::AgentSessionManager;
use crate::agent::state::AgentSession;
use crate::models::chat::Message;
use crate::repositories::MessageRepository;
use crate::search::message_index::{MessageSearchEngine, SearchResult};
use crate::state::get_message_repository;
use crate::utils::pagination::{paginate_in_memory, Page};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Global cache for loaded search indices (session_id -> MessageSearchEngine)
static INDEX_CACHE: once_cell::sync::Lazy<Mutex<HashMap<String, MessageSearchEngine>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

pub struct MessageService;

impl MessageService {
    pub fn filter_duplicate_injected_messages(
        existing_messages: &[Message],
        incoming_messages: &[Message],
    ) -> Vec<Message> {
        let mut known_message_ids = existing_messages
            .iter()
            .map(|message| message.id.clone())
            .collect::<std::collections::HashSet<_>>();

        incoming_messages
            .iter()
            .filter_map(|message| {
                if known_message_ids.insert(message.id.clone()) {
                    Some(message.clone())
                } else {
                    None
                }
            })
            .collect()
    }

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

    /// Queues a user message for a busy session.
    /// It adds the message to `pending_events` and persists it to the database,
    /// but explicitly does NOT touch `session.messages` to preserve the active LLM context window.
    pub async fn queue_user_message(
        active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
        session_id: &str,
        user_message: &Message,
    ) -> Result<(), String> {
        // 1. Add to pending events only
        {
            let sessions = active_sessions.read().await;
            if let Some(session) = sessions.get(session_id) {
                let mut pending = session.pending_events.write().await;
                pending.add(crate::agent::state::PendingEvent::Message(
                    user_message.id.clone(),
                ));
            } else {
                return Err(format!("Session not found: {}", session_id));
            }
        }

        // 2. Persist to DB synchronously
        let repo = get_message_repository();
        if let Err(e) = repo.insert(user_message).await {
            log::error!(
                "Failed to save queued user message to DB: session={}, msg_id={}, error={}",
                session_id,
                user_message.id,
                e
            );
            return Err(format!("Failed to persist queued message: {}", e));
        }

        Ok(())
    }

    /// Appends a user message to the session cache and persists it to the database.
    /// This handles deduplication, caching, UI event emission, and DB persistence
    /// explicitly for the `start_workflow` execution path.
    /// Callers must ensure `ensure_cache_initialized` has been called before invoking this.
    pub async fn append_user_message_to_session(
        active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
        app_handle: &AppHandle,
        session_id: &str,
        user_message: &Message,
    ) -> Result<(), String> {
        // 1. Add user message to in-memory cache FIRST (immediate, non-blocking)
        {
            let sessions = active_sessions.read().await;
            let session = sessions
                .get(session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;

            let mut messages = session.messages.write().await;

            // Deduplicate: Check if message ID already exists
            if messages.iter().any(|m| m.id == user_message.id) {
                log::warn!(
                    "Ignoring duplicate user message in session cache: {}",
                    user_message.id
                );
                return Ok(());
            }

            messages.push(user_message.clone());

            // Apply sliding window policy
            if messages.len() > crate::agent::state::MAX_CACHED_MESSAGES {
                let removed = messages.remove(0);
                log::debug!("Sliding window evicted: {}", removed.id);
            }

            log::info!(
                "📥 Message stack after user message: session={}, count={}, latest_message={}",
                session_id,
                messages.len(),
                user_message.id
            );
        } // Lock released

        // 2. Emit UI event (immediate)
        let message_added_event = crate::agent::events::AgentEvent::MessageAdded {
            session_id: session_id.to_string(),
            message: Box::new(user_message.clone()),
        };
        crate::agent::events::emit_agent_event(app_handle, message_added_event)
            .map_err(|e| format!("Failed to emit MessageAdded event: {}", e))?;

        // 3. Persist to DB synchronously to ensure data integrity
        let repo = get_message_repository();
        if let Err(e) = repo.insert(user_message).await {
            log::error!(
                "Failed to save user message to DB: session={}, msg_id={}, error={}",
                session_id,
                user_message.id,
                e
            );
            return Err(format!("Failed to persist message: {}", e));
        }

        Ok(())
    }

    /// Injects messages into a session cache, optionally triggering events immediately
    /// or queueing them for a running workflow.
    pub async fn inject_messages_to_session(
        active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
        app_handle: &AppHandle,
        session_id: &str,
        messages: Vec<Message>,
        emit_events_immediately: bool,
    ) -> Result<(), String> {
        // 1. Ensure cache is initialized
        crate::agent::lifecycle::ensure_cache_initialized(active_sessions, session_id).await?;

        // 2. Get session reference — note: multiple nested locks are acquired below
        // (sessions read-guard, then session.messages write-guard and/or session.pending_events write-guard)
        let sessions = active_sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let mut accepted_messages = Vec::new();

        // 3. Add messages to in-memory cache
        {
            let mut session_messages = session.messages.write().await;
            let deduped_messages =
                Self::filter_duplicate_injected_messages(&session_messages, &messages);
            let deduped_message_ids = deduped_messages
                .iter()
                .map(|message| message.id.as_str())
                .collect::<std::collections::HashSet<_>>();

            for msg in &messages {
                if !deduped_message_ids.contains(msg.id.as_str()) {
                    log::warn!(
                        "Skipping duplicate injected message for session {}: {}",
                        session_id,
                        msg.id
                    );
                }
            }

            for msg in &deduped_messages {
                session_messages.push(msg.clone());
                accepted_messages.push(msg.clone());
                if session_messages.len() > crate::agent::state::MAX_CACHED_MESSAGES {
                    session_messages.remove(0);
                }
            }
        }

        if accepted_messages.is_empty() {
            drop(sessions);
            return Ok(());
        }

        // 4. Emit MessageAdded events immediately, or queue as pending for the running workflow
        if emit_events_immediately {
            // Drop session lock before I/O operations
            drop(sessions);

            for msg in &accepted_messages {
                let event = crate::agent::events::AgentEvent::MessageAdded {
                    session_id: session_id.to_string(),
                    message: Box::new(msg.clone()),
                };
                crate::agent::events::emit_agent_event(app_handle, event)
                    .map_err(|e| format!("Failed to emit MessageAdded event: {}", e))?;
            }
        } else {
            // Track these message IDs as pending (will emit when workflow picks them up)
            let mut pending_events = session.pending_events.write().await;
            for msg in &accepted_messages {
                pending_events.add(crate::agent::state::PendingEvent::Message(msg.id.clone()));
            }
            log::info!(
                "Marked {} messages as pending for session: {} (IDs: {:?})",
                accepted_messages.len(),
                session_id,
                accepted_messages.iter().map(|m| &m.id).collect::<Vec<_>>()
            );
            drop(pending_events);
            drop(sessions);
        }

        // 5. Persist to DB asynchronously
        let msgs_for_db = accepted_messages.clone();
        tokio::spawn(async move {
            let repo = get_message_repository();
            for msg in msgs_for_db {
                if let Err(e) = repo.insert(&msg).await {
                    log::error!("Failed to inject message to DB: {}", e);
                }
            }
        });

        Ok(())
    }
}
