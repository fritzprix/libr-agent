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

/// Compares two messages for content equality to detect duplicate user messages.
/// - ⚠️ Designed to work ONLY with user-role messages (user-message-only).
/// - Returns false if either message is not role "user".
/// - Returns false if attachments differ.
/// - Normalizes text content (collapses whitespace) before comparison.
/// - Returns false if content contains non-Text variants (e.g., Image) for safety.
fn messages_content_equal(a: &Message, b: &Message) -> bool {
    if a.role != b.role || a.role != "user" {
        return false;
    }

    if a.attachments != b.attachments {
        return false;
    }

    if a.content.len() != b.content.len() {
        return false;
    }

    for (ca, cb) in a.content.iter().zip(b.content.iter()) {
        match (ca, cb) {
            (
                crate::mcp::types::MCPContent::Text { text: ta, .. },
                crate::mcp::types::MCPContent::Text { text: tb, .. },
            ) => {
                let normalize = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
                if normalize(ta) != normalize(tb) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}
fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Generates a normalized signature for a message to detect consecutive duplicates.
/// Compares content items, thinking, tool_call_id, and tool calls while ignoring volatile fields.
///
/// `tool_call_id` must be part of the signature: loop-prevention (and similar) can emit
/// multiple tool results with identical text for different calls in one batch. Dropping
/// those breaks the tool_call → tool_result chain and leaves the UI in a loading state.
pub(crate) fn message_signature(msg: &Message) -> Option<String> {
    let mut parts = Vec::new();

    // 0. tool_call_id — distinguishes otherwise-identical tool result messages
    if let Some(tool_call_id) = &msg.tool_call_id {
        parts.push(format!("tool_call_id:{}", tool_call_id));
    }

    // 1. content (Text, Thinking, Image, Audio, Resource, ToolCall)
    for c in &msg.content {
        match c {
            crate::mcp::types::MCPContent::Text { text, .. } => {
                parts.push(format!("text:{}", normalize_whitespace(text)));
            }
            crate::mcp::types::MCPContent::Thinking { thinking, .. } => {
                parts.push(format!("thinking:{}", normalize_whitespace(thinking)));
            }
            crate::mcp::types::MCPContent::Image {
                data,
                uri,
                mime_type,
            } => {
                parts.push(format!(
                    "image:{}:{}:{}",
                    data.as_deref().unwrap_or(""),
                    uri.as_deref().unwrap_or(""),
                    mime_type
                ));
            }
            crate::mcp::types::MCPContent::Audio {
                data,
                uri,
                mime_type,
            } => {
                parts.push(format!(
                    "audio:{}:{}:{}",
                    data.as_deref().unwrap_or(""),
                    uri.as_deref().unwrap_or(""),
                    mime_type
                ));
            }
            crate::mcp::types::MCPContent::Resource { resource, .. } => {
                parts.push(format!(
                    "resource:{}",
                    serde_json::to_string(resource).unwrap_or_default()
                ));
            }
            crate::mcp::types::MCPContent::ToolCall {
                name, arguments, ..
            } => {
                parts.push(format!(
                    "toolcall_content:{}:{}",
                    name,
                    normalize_whitespace(arguments)
                ));
            }
        }
    }

    // 2. thinking 별도 필드
    if let Some(t) = &msg.thinking {
        parts.push(format!("thinking_field:{}", normalize_whitespace(t)));
    }

    // 3. tool_calls
    if let Some(tcs) = &msg.tool_calls {
        for tc in tcs {
            parts.push(format!(
                "toolcall:{}:{}",
                tc.function.name,
                normalize_whitespace(&tc.function.arguments)
            ));
        }
    }

    if parts.is_empty() {
        return None;
    }
    Some(parts.join("|||"))
}

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
        crate::agent::tauri_events::emit_agent_event(app_handle, message_added_event)
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
        mut messages: Vec<Message>,
        emit_events_immediately: bool,
    ) -> Result<(), String> {
        crate::agent::lifecycle::ensure_cache_initialized(active_sessions, session_id).await?;

        let sessions = active_sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        // ── Phase 1: Message dedup + In-memory push ───────────────────────
        {
            // 1a. Capture last-message signature while holding the lock
            let last_msg_sig = {
                let session_messages = session.messages.read().await;
                session_messages.last().and_then(message_signature)
            };

            // 1b. Filter out repeated messages (no lock needed)
            // Note: User messages are bypassed here as they have custom pop-and-replace
            // logic in Phase 1c and need to be recorded in the database.
            let mut current_last_sig = last_msg_sig;
            messages.retain(|msg| {
                if msg.role == "user" {
                    if let Some(sig) = message_signature(msg) {
                        current_last_sig = Some(sig);
                    }
                    return true;
                }
                let sig = message_signature(msg);
                if let Some(ref last_sig) = current_last_sig {
                    if sig == Some(last_sig.clone()) {
                        log::info!(
                            "Skipping repeated message: session={}, msg_id={}, role={}",
                            session_id,
                            msg.id,
                            msg.role
                        );
                        return false;
                    }
                }
                if let Some(ref s) = sig {
                    current_last_sig = Some(s.clone());
                }
                true
            });

            // Early return if all messages were deduped
            if messages.is_empty() {
                return Ok(());
            }

            // 1c. Push remaining messages (re-acquire write lock)
            {
                let mut session_messages = session.messages.write().await;

                for msg in &messages {
                    // Trailing duplicate user messages discard
                    if msg.role == "user" {
                        while let Some(last) = session_messages.last() {
                            if !messages_content_equal(msg, last) {
                                break;
                            }
                            session_messages.pop();
                            log::info!(
                                "Discarded trailing duplicate user message (content match): \
                                 session={}, new={}",
                                session_id,
                                msg.id
                            );
                        }
                    }

                    session_messages.push(msg.clone());

                    if session_messages.len() > crate::agent::state::MAX_CACHED_MESSAGES {
                        let removed = session_messages.remove(0);
                        log::debug!("Evicted from sliding window cache: {}", removed.id);
                    }
                }
            }
        } // session.messages lock released

        // ── Phase 2: Persist to DB (SYNC — crash-safe) ──────────────────────
        {
            let repo = get_message_repository();
            for msg in &messages {
                if let Err(e) = repo.insert(msg).await {
                    log::error!(
                        "Failed to persist injected message to DB: session={}, msg_id={}, error={}",
                        session_id,
                        msg.id,
                        e
                    );
                    return Err(format!("Failed to persist message: {}", e));
                }
            }
        }

        // ── Phase 3: Emit UI events ─────────────────────────────────────────
        if emit_events_immediately {
            drop(sessions);
            for msg in &messages {
                let event = crate::agent::events::AgentEvent::MessageAdded {
                    session_id: session_id.to_string(),
                    message: Box::new(msg.clone()),
                };
                crate::agent::tauri_events::emit_agent_event(app_handle, event)
                    .map_err(|e| format!("Failed to emit MessageAdded event: {}", e))?;
            }
        } else {
            let mut pending_events = session.pending_events.write().await;
            for msg in &messages {
                pending_events.add(crate::agent::state::PendingEvent::Message(msg.id.clone()));
            }
            log::info!(
                "Marked {} messages as pending for session: {} (IDs: {:?})",
                messages.len(),
                session_id,
                messages.iter().map(|m| &m.id).collect::<Vec<_>>()
            );
            drop(pending_events);
        }

        Ok(())
    }
}
