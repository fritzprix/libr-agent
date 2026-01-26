use crate::agent::session_manager::AgentSessionManager;
use crate::agent::types::ToolCall;
use crate::mcp::types::MCPContent;
use crate::repositories::MessageRepository;
use crate::search::index_storage::{get_index_path, write_index_atomic, IndexData, IndexMetadata};
use crate::search::message_index::{MessageSearchEngine, SearchResult};
use crate::state::get_message_repository;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::command;
use tauri::State;

/// Default timestamp generator for serde deserialization fallback
fn default_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Generic pagination wrapper for query results.
/// This type is shared across all paginated responses in the application.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page: usize,
    pub page_size: usize,
    pub total_items: usize,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}

/// Message data model matching the frontend TypeScript Message interface.
/// Stores chat messages for sessions with support for various content types.
/// All fields use structured types - JSON serialization handled in Repository layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,
    /// Structured content array (MCPContent[]) - matches TypeScript
    pub content: Vec<MCPContent>,
    /// Tool calls as structured array - matches TypeScript
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub is_streaming: Option<bool>,
    pub thinking: Option<String>,
    pub thinking_signature: Option<String>,
    pub assistant_id: Option<String>,
    /// Attachments as structured value
    pub attachments: Option<serde_json::Value>,
    /// Tool use as structured value
    pub tool_use: Option<serde_json::Value>,
    #[serde(default = "default_timestamp")]
    pub created_at: i64, // Unix timestamp in milliseconds
    #[serde(default = "default_timestamp")]
    pub updated_at: i64, // Unix timestamp in milliseconds
    pub source: Option<String>,
    /// Error information as structured value
    pub error: Option<serde_json::Value>,
}

// The database layer has been migrated to repositories/message_repository.rs
// All database operations now go through the MessageRepository trait

// ========================================
// Tauri Commands
// ========================================

/// Get a paginated list of messages for a session.
#[command]
pub async fn messages_get_page(
    session_id: String,
    page: usize,
    page_size: usize,
) -> Result<Page<Message>, String> {
    let repo = get_message_repository();
    repo.get_page(&session_id, page, page_size)
        .await
        .map_err(|e| e.to_string())
}

/// Insert or update multiple messages at once.
#[command]
pub async fn messages_upsert_many(messages: Vec<Message>) -> Result<(), String> {
    let repo = get_message_repository();
    repo.insert_many(messages).await.map_err(|e| e.to_string())
}

/// Insert or update a single message.
#[command]
pub async fn messages_upsert(message: Message) -> Result<(), String> {
    let repo = get_message_repository();
    repo.insert(&message).await.map_err(|e| e.to_string())
}

/// Delete a single message by ID.
/// Also removes the message from the in-memory cache of active sessions to maintain consistency.
#[command]
pub async fn messages_delete(
    message_id: String,
    session_manager: State<'_, AgentSessionManager>,
) -> Result<(), String> {
    // First, get the message to find which session it belongs to
    let repo = get_message_repository();

    // Find the message's session_id before deleting
    // We need to load it to identify which session cache to update
    let page = repo
        .get_page("", 1, 10000) // This is inefficient but messages_delete doesn't have session context
        .await
        .map_err(|e| format!("Failed to query messages: {}", e))?;

    let target_message = page.items.iter().find(|m| m.id == message_id);
    let session_id = target_message.map(|m| m.session_id.clone());

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

/// Delete all messages for a specific session.
#[command]
pub async fn messages_delete_all_for_session(session_id: String) -> Result<(), String> {
    let repo = get_message_repository();
    repo.delete_by_session(&session_id)
        .await
        .map_err(|e| e.to_string())
}

// ========================================
// Search Functionality
// ========================================

/// Global cache for loaded search indices (session_id -> MessageSearchEngine)
static INDEX_CACHE: once_cell::sync::Lazy<Mutex<HashMap<String, MessageSearchEngine>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

/// Load or rebuild the search index for a session.
async fn get_or_build_index(session_id: &str) -> Result<MessageSearchEngine, String> {
    let repo = get_message_repository();
    let index_path = get_index_path(session_id)?;
    let max_docs = MessageSearchEngine::max_docs_from_env();

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
    let start_time = std::time::Instant::now();

    // Fetch messages from database (most recent max_docs)
    let messages = crate::get_message_repository()
        .get_message_models_by_session(session_id, max_docs as u64)
        .await
        .map_err(|e| format!("Failed to fetch messages for indexing: {e}"))?;

    // Build index
    let engine =
        MessageSearchEngine::build_from_models(session_id.to_string(), messages, max_docs)?;

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
#[command]
pub async fn messages_search(
    query: String,
    session_id: Option<String>,
    page: usize,
    page_size: usize,
) -> Result<Page<SearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(Page {
            items: Vec::new(),
            page,
            page_size,
            total_items: 0,
            has_next_page: false,
            has_previous_page: false,
        });
    }

    // If a session_id was provided, use the per-session cached index.
    // Otherwise perform a global search by building a temporary index over all messages.
    let all_results = if let Some(target_session) = session_id {
        // Per-session behavior (cached index)
        let engine = get_or_build_index(&target_session).await?;
        engine.search(&query, page * page_size * 2)?
    } else {
        // Global search: build a temporary index from messages across all sessions.
        let max_docs = MessageSearchEngine::max_docs_from_env();

        // Fetch recent messages across all sessions up to max_docs
        let messages = crate::get_message_repository()
            .get_recent_message_models(max_docs as u64)
            .await
            .map_err(|e| format!("Failed to fetch messages for global indexing: {e}"))?;

        // Perform search on the temporary engine
        let engine =
            MessageSearchEngine::build_from_models("global".to_string(), messages, max_docs)?;
        engine.search(&query, page * page_size * 2)?
    };

    // Paginate results
    let total_items = all_results.len();
    let start_idx = (page.saturating_sub(1)) * page_size;
    let end_idx = (start_idx + page_size).min(total_items);

    let items: Vec<SearchResult> = if start_idx < total_items {
        all_results[start_idx..end_idx].to_vec()
    } else {
        Vec::new()
    };

    Ok(Page {
        items,
        page,
        page_size,
        total_items,
        has_next_page: end_idx < total_items,
        has_previous_page: page > 1,
    })
}
