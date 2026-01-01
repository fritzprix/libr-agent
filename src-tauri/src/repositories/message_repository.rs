use super::error::DbError;
use crate::commands::messages_commands::{Message, Page};
use async_trait::async_trait;
use sqlx::{Row, SqlitePool};

/// Message repository trait for abstraction and testability
#[async_trait]
pub trait MessageRepository: Send + Sync {
    /// Initialize the messages table and indexes
    async fn create_table(&self) -> Result<(), DbError>;

    /// Retrieve a paginated list of messages for a specific session
    async fn get_page(
        &self,
        session_id: &str,
        page: usize,
        page_size: usize,
    ) -> Result<Page<Message>, DbError>;

    /// Insert or update a single message
    async fn insert(&self, message: &Message) -> Result<(), DbError>;

    /// Insert or update multiple messages in a transaction
    async fn insert_many(&self, messages: Vec<Message>) -> Result<(), DbError>;

    /// Delete a single message by its ID
    async fn delete_by_id(&self, message_id: &str) -> Result<(), DbError>;

    /// Delete all messages for a specific session
    async fn delete_by_session(&self, session_id: &str) -> Result<(), DbError>;

    /// Update index metadata after rebuilding
    async fn update_index_meta(
        &self,
        session_id: &str,
        index_path: &str,
        doc_count: usize,
        rebuild_duration_ms: i64,
    ) -> Result<(), DbError>;

    /// Get the last indexed timestamp for a session
    async fn get_last_indexed_at(&self, session_id: &str) -> Result<i64, DbError>;

    /// Check if a session has messages newer than the last index build
    async fn is_index_dirty(&self, session_id: &str) -> Result<bool, DbError>;
}

/// SQLite implementation of MessageRepository
#[derive(Debug)]
pub struct SqliteMessageRepository {
    pool: SqlitePool,
}

impl SqliteMessageRepository {
    /// Create a new SQLite message repository with the given pool
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MessageRepository for SqliteMessageRepository {
    async fn create_table(&self) -> Result<(), DbError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL CHECK(session_id <> ''),
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
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_page(
        &self,
        session_id: &str,
        page: usize,
        page_size: usize,
    ) -> Result<Page<Message>, DbError> {
        if page_size == 0 {
            return Err(DbError::InvalidInput("page_size must be > 0".into()));
        }

        let offset = (page.saturating_sub(1)) as i64 * page_size as i64;

        // Get total count
        let row = sqlx::query("SELECT COUNT(1) as count FROM messages WHERE session_id = ?")
            .bind(session_id)
            .fetch_one(&self.pool)
            .await?;
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
        .fetch_all(&self.pool)
        .await?;

        let messages: Vec<Message> = rows
            .into_iter()
            .map(|row| {
                // Deserialize JSON strings from DB into structured types
                let content_str: String = row.get("content");
                let content: Vec<crate::mcp::types::MCPContent> =
                    serde_json::from_str(&content_str).unwrap_or_default();

                let tool_calls: Option<Vec<crate::agent::types::ToolCall>> = row
                    .get::<Option<String>, _>("tool_calls")
                    .and_then(|s| serde_json::from_str(&s).ok());

                let attachments: Option<serde_json::Value> = row
                    .get::<Option<String>, _>("attachments")
                    .and_then(|s| serde_json::from_str(&s).ok());

                let tool_use: Option<serde_json::Value> = row
                    .get::<Option<String>, _>("tool_use")
                    .and_then(|s| serde_json::from_str(&s).ok());

                let error: Option<serde_json::Value> = row
                    .get::<Option<String>, _>("error")
                    .and_then(|s| serde_json::from_str(&s).ok());

                Message {
                    id: row.get("id"),
                    session_id: row.get("session_id"),
                    role: row.get("role"),
                    content,
                    tool_calls,
                    tool_call_id: row.get("tool_call_id"),
                    is_streaming: row.get("is_streaming"),
                    thinking: row.get("thinking"),
                    thinking_signature: row.get("thinking_signature"),
                    assistant_id: row.get("assistant_id"),
                    attachments,
                    tool_use,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                    source: row.get("source"),
                    error,
                }
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

    async fn insert(&self, message: &Message) -> Result<(), DbError> {
        // Serialize structured types to JSON strings for DB storage
        let content_json = serde_json::to_string(&message.content).map_err(|e| {
            DbError::SerializationError(format!("Failed to serialize content: {}", e))
        })?;

        let tool_calls_json = message
            .tool_calls
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| {
                DbError::SerializationError(format!("Failed to serialize tool_calls: {}", e))
            })?;

        let attachments_json = message
            .attachments
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| {
                DbError::SerializationError(format!("Failed to serialize attachments: {}", e))
            })?;

        let tool_use_json = message
            .tool_use
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| {
                DbError::SerializationError(format!("Failed to serialize tool_use: {}", e))
            })?;

        let error_json = message
            .error
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| {
                DbError::SerializationError(format!("Failed to serialize error: {}", e))
            })?;

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
        .bind(content_json)
        .bind(tool_calls_json)
        .bind(&message.tool_call_id)
        .bind(message.is_streaming)
        .bind(&message.thinking)
        .bind(&message.thinking_signature)
        .bind(&message.assistant_id)
        .bind(attachments_json)
        .bind(tool_use_json)
        .bind(message.created_at)
        .bind(message.updated_at)
        .bind(&message.source)
        .bind(error_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn insert_many(&self, messages: Vec<Message>) -> Result<(), DbError> {
        let mut tx = self.pool.begin().await?;

        for message in messages {
            // Serialize structured types to JSON strings for DB storage
            let content_json = serde_json::to_string(&message.content).map_err(|e| {
                DbError::SerializationError(format!("Failed to serialize content: {}", e))
            })?;

            let tool_calls_json = message
                .tool_calls
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|e| {
                    DbError::SerializationError(format!("Failed to serialize tool_calls: {}", e))
                })?;

            let attachments_json = message
                .attachments
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|e| {
                    DbError::SerializationError(format!("Failed to serialize attachments: {}", e))
                })?;

            let tool_use_json = message
                .tool_use
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|e| {
                    DbError::SerializationError(format!("Failed to serialize tool_use: {}", e))
                })?;

            let error_json = message
                .error
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(|e| {
                    DbError::SerializationError(format!("Failed to serialize error: {}", e))
                })?;

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
            .bind(content_json)
            .bind(tool_calls_json)
            .bind(&message.tool_call_id)
            .bind(message.is_streaming)
            .bind(&message.thinking)
            .bind(&message.thinking_signature)
            .bind(&message.assistant_id)
            .bind(attachments_json)
            .bind(tool_use_json)
            .bind(message.created_at)
            .bind(message.updated_at)
            .bind(&message.source)
            .bind(error_json)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit()
            .await
            .map_err(|e| DbError::TransactionFailed(e.to_string()))?;
        Ok(())
    }

    async fn delete_by_id(&self, message_id: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM messages WHERE id = ?")
            .bind(message_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_by_session(&self, session_id: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM messages WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn update_index_meta(
        &self,
        session_id: &str,
        index_path: &str,
        doc_count: usize,
        rebuild_duration_ms: i64,
    ) -> Result<(), DbError> {
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
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_last_indexed_at(&self, session_id: &str) -> Result<i64, DbError> {
        let result =
            sqlx::query("SELECT last_indexed_at FROM message_index_meta WHERE session_id = ?")
                .bind(session_id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(result.map(|row| row.get("last_indexed_at")).unwrap_or(0))
    }

    async fn is_index_dirty(&self, session_id: &str) -> Result<bool, DbError> {
        let last_indexed_at = self.get_last_indexed_at(session_id).await?;

        let row =
            sqlx::query("SELECT MAX(created_at) as max_created FROM messages WHERE session_id = ?")
                .bind(session_id)
                .fetch_one(&self.pool)
                .await?;

        let max_created: Option<i64> = row.get("max_created");
        Ok(max_created.map(|t| t > last_indexed_at).unwrap_or(false))
    }
}
