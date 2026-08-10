use async_trait::async_trait;

use crate::entity::message;
use crate::models::chat::Message;
use crate::utils::pagination::Page;

use super::super::error::DbError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessagePaginationCursor {
    pub created_at: i64,
    pub row_id: i64,
}

#[derive(Debug, Clone)]
pub struct MessageSlicePage {
    pub items: Vec<Message>,
    pub has_more_before: bool,
    pub oldest_cursor: Option<MessagePaginationCursor>,
}

#[derive(Debug)]
pub(super) struct MessageRowWithCursor {
    pub(super) model: message::Model,
    pub(super) cursor: MessagePaginationCursor,
}

/// Message repository trait for abstraction and testability
#[async_trait]
pub trait MessageRepository: Send + Sync {
    /// Retrieve a paginated list of messages for a specific session
    async fn get_page(
        &self,
        session_id: &str,
        page: u64,
        page_size: u64,
    ) -> Result<Page<Message>, DbError>;

    /// Insert or update a single message
    async fn insert(&self, message: &Message) -> Result<(), DbError>;

    /// Insert or update multiple messages in a transaction
    async fn insert_many(&self, messages: Vec<Message>) -> Result<(), DbError>;

    /// Retrieve a single message by its ID
    async fn get_by_id(&self, message_id: &str) -> Result<Option<Message>, DbError>;

    /// Retrieve multiple messages by their IDs
    async fn get_by_ids(&self, message_ids: Vec<String>) -> Result<Vec<Message>, DbError>;

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

    /// Delete index metadata for a specific session
    async fn delete_index_metadata(&self, session_id: &str) -> Result<(), DbError>;

    /// Get recent messages for a specific session with limit
    async fn get_messages_by_session(
        &self,
        session_id: &str,
        limit: u64,
    ) -> Result<Vec<Message>, DbError>;

    /// Get the most recent messages for a session in ascending chronological order.
    /// Returns `limit + 1` internally so callers can infer whether older messages exist.
    async fn get_recent_slice(
        &self,
        session_id: &str,
        limit: u64,
    ) -> Result<MessageSlicePage, DbError>;

    /// Get messages older than a row-id cursor for a session in ascending causal order.
    /// Returned cursors still include `created_at` for UI metadata, but row_id is the ordering
    /// truth so message history is resilient to clock skew or cross-layer timestamp drift.
    async fn get_messages_before(
        &self,
        session_id: &str,
        before_row_id: i64,
        limit: u64,
    ) -> Result<MessageSlicePage, DbError>;

    /// Get recent messages across all sessions with limit
    async fn get_recent_messages(&self, limit: u64) -> Result<Vec<Message>, DbError>;

    /// Get all distinct session IDs that have messages
    async fn get_distinct_sessions(&self) -> Result<Vec<String>, DbError>;

    /// Get message counts grouped by session ID
    async fn count_by_session(&self) -> Result<Vec<(String, u64)>, DbError>;

    /// Get message models (raw SeaORM models) for search indexing
    async fn get_message_models_by_session(
        &self,
        session_id: &str,
        limit: u64,
    ) -> Result<Vec<message::Model>, DbError>;

    /// Get recent message models across all sessions for search indexing
    async fn get_recent_message_models(&self, limit: u64) -> Result<Vec<message::Model>, DbError>;
}
