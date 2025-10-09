use crate::state::get_sqlite_pool;
use serde::{Deserialize, Serialize};
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
