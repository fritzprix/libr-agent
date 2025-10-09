use crate::search::index_storage::{get_index_path, write_index_atomic, IndexData, IndexMetadata};
use crate::search::message_index::{MessageDocument, MessageSearchEngine, SearchResult};
use crate::state::get_sqlite_pool;
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

/// Database layer functions for messages table
pub mod db {
    use super::{Message, Page};
    use sqlx::{Row, SqlitePool};

    /// Initialize the messages table in the database.
    /// Creates the table and indexes if they don't already exist.
    pub async fn create_messages_table(pool: &SqlitePool) -> Result<(), String> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                tool_calls TEXT,
                tool_call_id TEXT,
                is_streaming INTEGER,
                thinking TEXT,
                thinking_signature TEXT,
                assistant_id TEXT,
                attachments TEXT,
                tool_use TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                source TEXT,
                error TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_messages_session_created
            ON messages(session_id, created_at);

            CREATE INDEX IF NOT EXISTS idx_messages_session_id
            ON messages(session_id);

            CREATE TABLE IF NOT EXISTS message_index_meta (
                session_id TEXT PRIMARY KEY,
                index_path TEXT,
                last_indexed_at INTEGER DEFAULT 0,
                doc_count INTEGER DEFAULT 0,
                index_version INTEGER DEFAULT 1,
                last_rebuild_duration_ms INTEGER
            );
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create messages table: {e}"))?;

        Ok(())
    }

    /// Retrieve a paginated list of messages for a specific session.
    /// Messages are ordered by created_at in ascending order (oldest first).
    pub async fn get_messages_page(
        pool: &SqlitePool,
        session_id: &str,
        page: usize,
        page_size: usize,
    ) -> Result<Page<Message>, String> {
        if page_size == 0 {
            return Err("page_size must be > 0".to_string());
        }

        let offset = (page.saturating_sub(1)) as i64 * page_size as i64;

        // Get total count
        let row = sqlx::query("SELECT COUNT(1) as count FROM messages WHERE session_id = ?")
            .bind(session_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to count messages: {e}"))?;
        let total: i64 = row.get("count");

        // Fetch paginated messages
        let rows = sqlx::query(
            r#"
            SELECT
                id, session_id, role, content, tool_calls, tool_call_id,
                is_streaming, thinking, thinking_signature, assistant_id,
                attachments, tool_use, created_at, updated_at, source, error
            FROM messages
            WHERE session_id = ?
            ORDER BY created_at ASC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(session_id)
        .bind(page_size as i64)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch messages: {e}"))?;

        let messages: Vec<Message> = rows
            .into_iter()
            .map(|row| Message {
                id: row.get("id"),
                session_id: row.get("session_id"),
                role: row.get("role"),
                content: row.get("content"),
                tool_calls: row.get("tool_calls"),
                tool_call_id: row.get("tool_call_id"),
                is_streaming: row.get("is_streaming"),
                thinking: row.get("thinking"),
                thinking_signature: row.get("thinking_signature"),
                assistant_id: row.get("assistant_id"),
                attachments: row.get("attachments"),
                tool_use: row.get("tool_use"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                source: row.get("source"),
                error: row.get("error"),
            })
            .collect();

        let total_usize = total as usize;
        let has_prev = page > 1;
        let has_next = page * page_size < total_usize;

        Ok(Page {
            items: messages,
            page,
            page_size,
            total_items: total_usize,
            has_next_page: has_next,
            has_previous_page: has_prev,
        })
    }

    /// Insert or update a single message in the database.
    pub async fn upsert_message(pool: &SqlitePool, message: Message) -> Result<(), String> {
        sqlx::query(
            r#"
            INSERT INTO messages (
                id, session_id, role, content, tool_calls, tool_call_id,
                is_streaming, thinking, thinking_signature, assistant_id,
                attachments, tool_use, created_at, updated_at, source, error
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                session_id = excluded.session_id,
                role = excluded.role,
                content = excluded.content,
                tool_calls = excluded.tool_calls,
                tool_call_id = excluded.tool_call_id,
                is_streaming = excluded.is_streaming,
                thinking = excluded.thinking,
                thinking_signature = excluded.thinking_signature,
                assistant_id = excluded.assistant_id,
                attachments = excluded.attachments,
                tool_use = excluded.tool_use,
                updated_at = excluded.updated_at,
                source = excluded.source,
                error = excluded.error
            "#,
        )
        .bind(&message.id)
        .bind(&message.session_id)
        .bind(&message.role)
        .bind(&message.content)
        .bind(&message.tool_calls)
        .bind(&message.tool_call_id)
        .bind(message.is_streaming)
        .bind(&message.thinking)
        .bind(&message.thinking_signature)
        .bind(&message.assistant_id)
        .bind(&message.attachments)
        .bind(&message.tool_use)
        .bind(message.created_at)
        .bind(message.updated_at)
        .bind(&message.source)
        .bind(&message.error)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to upsert message: {e}"))?;

        Ok(())
    }

    /// Insert or update multiple messages in a single transaction.
    pub async fn upsert_messages(pool: &SqlitePool, messages: Vec<Message>) -> Result<(), String> {
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| format!("Failed to start transaction: {e}"))?;

        for message in messages {
            sqlx::query(
                r#"
                INSERT INTO messages (
                    id, session_id, role, content, tool_calls, tool_call_id,
                    is_streaming, thinking, thinking_signature, assistant_id,
                    attachments, tool_use, created_at, updated_at, source, error
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    session_id = excluded.session_id,
                    role = excluded.role,
                    content = excluded.content,
                    tool_calls = excluded.tool_calls,
                    tool_call_id = excluded.tool_call_id,
                    is_streaming = excluded.is_streaming,
                    thinking = excluded.thinking,
                    thinking_signature = excluded.thinking_signature,
                    assistant_id = excluded.assistant_id,
                    attachments = excluded.attachments,
                    tool_use = excluded.tool_use,
                    updated_at = excluded.updated_at,
                    source = excluded.source,
                    error = excluded.error
                "#,
            )
            .bind(&message.id)
            .bind(&message.session_id)
            .bind(&message.role)
            .bind(&message.content)
            .bind(&message.tool_calls)
            .bind(&message.tool_call_id)
            .bind(message.is_streaming)
            .bind(&message.thinking)
            .bind(&message.thinking_signature)
            .bind(&message.assistant_id)
            .bind(&message.attachments)
            .bind(&message.tool_use)
            .bind(message.created_at)
            .bind(message.updated_at)
            .bind(&message.source)
            .bind(&message.error)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to upsert message in transaction: {e}"))?;
        }

        tx.commit()
            .await
            .map_err(|e| format!("Failed to commit transaction: {e}"))?;

        Ok(())
    }

    /// Delete a single message by its ID.
    pub async fn delete_message(pool: &SqlitePool, message_id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM messages WHERE id = ?")
            .bind(message_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to delete message: {e}"))?;

        Ok(())
    }

    /// Delete all messages for a specific session.
    /// Used when clearing session history or removing a session.
    pub async fn delete_all_for_session(pool: &SqlitePool, session_id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM messages WHERE session_id = ?")
            .bind(session_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to delete messages for session: {e}"))?;

        Ok(())
    }

    /// Update index metadata for a session after rebuilding.
    pub async fn update_index_meta(
        pool: &SqlitePool,
        session_id: &str,
        index_path: &str,
        doc_count: usize,
        rebuild_duration_ms: i64,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().timestamp_millis();

        sqlx::query(
            r#"
            INSERT INTO message_index_meta (session_id, index_path, last_indexed_at, doc_count, last_rebuild_duration_ms)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(session_id) DO UPDATE SET
                index_path = excluded.index_path,
                last_indexed_at = excluded.last_indexed_at,
                doc_count = excluded.doc_count,
                last_rebuild_duration_ms = excluded.last_rebuild_duration_ms
            "#,
        )
        .bind(session_id)
        .bind(index_path)
        .bind(now)
        .bind(doc_count as i64)
        .bind(rebuild_duration_ms)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to update index meta: {e}"))?;

        Ok(())
    }

    /// Get the last indexed timestamp for a session.
    pub async fn get_last_indexed_at(pool: &SqlitePool, session_id: &str) -> Result<i64, String> {
        let result =
            sqlx::query("SELECT last_indexed_at FROM message_index_meta WHERE session_id = ?")
                .bind(session_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| format!("Failed to get index meta: {e}"))?;

        Ok(result.map(|row| row.get("last_indexed_at")).unwrap_or(0))
    }

    /// Check if a session has messages newer than the last index build.
    pub async fn is_index_dirty(pool: &SqlitePool, session_id: &str) -> Result<bool, String> {
        let last_indexed_at = get_last_indexed_at(pool, session_id).await?;

        let row =
            sqlx::query("SELECT MAX(created_at) as max_created FROM messages WHERE session_id = ?")
                .bind(session_id)
                .fetch_one(pool)
                .await
                .map_err(|e| format!("Failed to check index dirty status: {e}"))?;

        let max_created: Option<i64> = row.get("max_created");

        Ok(max_created.map(|t| t > last_indexed_at).unwrap_or(false))
    }
}

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
    let pool = get_sqlite_pool();
    db::get_messages_page(pool, &session_id, page, page_size).await
}

/// Insert or update multiple messages at once.
#[command]
pub async fn messages_upsert_many(messages: Vec<Message>) -> Result<(), String> {
    let pool = get_sqlite_pool();
    db::upsert_messages(pool, messages).await
}

/// Insert or update a single message.
#[command]
pub async fn messages_upsert(message: Message) -> Result<(), String> {
    let pool = get_sqlite_pool();
    db::upsert_message(pool, message).await
}

/// Delete a single message by ID.
#[command]
pub async fn messages_delete(message_id: String) -> Result<(), String> {
    let pool = get_sqlite_pool();
    db::delete_message(pool, &message_id).await
}

/// Delete all messages for a specific session.
#[command]
pub async fn messages_delete_all_for_session(session_id: String) -> Result<(), String> {
    let pool = get_sqlite_pool();
    db::delete_all_for_session(pool, &session_id).await
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
    let index_path = get_index_path(session_id)?;
    let max_docs = MessageSearchEngine::max_docs_from_env();

    // Check if index exists and is up to date
    let is_dirty = db::is_index_dirty(pool, session_id).await?;

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
    db::update_index_meta(
        pool,
        session_id,
        &index_path.to_string_lossy(),
        engine.doc_count(),
        rebuild_duration,
    )
    .await?;

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

    // For now, only support single-session search (as per spec)
    let target_session = session_id.ok_or_else(|| "session_id is required".to_string())?;

    // Get or build index
    let engine = get_or_build_index(&target_session).await?;

    // Perform search (get more results than needed for pagination)
    let all_results = engine.search(&query, page * page_size * 2)?;

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
