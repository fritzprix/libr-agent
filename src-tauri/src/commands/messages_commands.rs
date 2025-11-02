use crate::repositories::MessageRepository;
use crate::search::index_storage::{get_index_path, write_index_atomic, IndexData, IndexMetadata};
use crate::search::message_index::{MessageDocument, MessageSearchEngine, SearchResult};
use crate::state::{get_message_repository, get_sqlite_pool};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::command;

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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,
    /// Stored as JSON string to support complex content structures (MCPContent[])
    pub content: String,
    /// Tool calls stored as JSON string
    pub tool_calls: Option<String>,
    pub tool_call_id: Option<String>,
    pub is_streaming: Option<bool>,
    pub thinking: Option<String>,
    pub thinking_signature: Option<String>,
    pub assistant_id: Option<String>,
    /// Attachments stored as JSON string (AttachmentReference[])
    pub attachments: Option<String>,
    /// Tool use stored as JSON string
    pub tool_use: Option<String>,
    pub created_at: i64, // Unix timestamp in milliseconds
    pub updated_at: i64, // Unix timestamp in milliseconds
    pub source: Option<String>,
    /// Error information stored as JSON string
    pub error: Option<String>,
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
#[command]
pub async fn messages_delete(message_id: String) -> Result<(), String> {
    let repo = get_message_repository();
    repo.delete_by_id(&message_id)
        .await
        .map_err(|e| e.to_string())
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
    let pool = get_sqlite_pool();
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
            id: row.get("id"),
            session_id: row.get("session_id"),
            content: row.get("content"),
            created_at: row.get("created_at"),
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
        let pool = get_sqlite_pool();
        let max_docs = MessageSearchEngine::max_docs_from_env();

        // Fetch recent messages across all sessions up to max_docs
        let messages = sqlx::query(
            r#"
            SELECT id, session_id, content, created_at
            FROM messages
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(max_docs as i64)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch messages for global indexing: {e}"))?;

        let documents: Vec<MessageDocument> = messages
            .into_iter()
            .map(|row| MessageDocument {
                id: row.get("id"),
                session_id: row.get("session_id"),
                content: row.get("content"),
                created_at: row.get("created_at"),
            })
            .collect();

        let mut engine = MessageSearchEngine::new("global".to_string(), max_docs);
        engine.add_documents(documents)?;

        // Perform search on the temporary engine
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
