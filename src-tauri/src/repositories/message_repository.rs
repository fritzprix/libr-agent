use super::error::DbError;
use crate::commands::messages_commands::{Message, Page};
use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};

use crate::entity::prelude::{Message as MessageEntity, MessageIndexMeta};
use crate::entity::{message, message_index_meta};
use crate::utils::json::{from_json_option, from_json_or_default, to_json_option};

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

/// SQLite implementation of MessageRepository using SeaORM
#[derive(Debug)]
pub struct SqliteMessageRepository {
    db: DatabaseConnection,
}

impl SqliteMessageRepository {
    /// Create a new SQLite message repository with the given database connection
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Convert SeaORM message model to Message type
    fn model_to_message(model: message::Model) -> Message {
        let content: Vec<crate::mcp::types::MCPContent> = from_json_or_default(&model.content);

        let tool_calls: Option<Vec<crate::agent::types::ToolCall>> =
            from_json_option(&model.tool_calls);

        let attachments: Option<serde_json::Value> = from_json_option(&model.attachments);

        let tool_use: Option<serde_json::Value> = from_json_option(&model.tool_use);

        let error: Option<serde_json::Value> = from_json_option(&model.error);

        Message {
            id: model.id,
            session_id: model.session_id,
            role: model.role,
            content,
            tool_calls,
            tool_call_id: model.tool_call_id,
            is_streaming: model.is_streaming.map(|v| v != 0),
            thinking: model.thinking,
            thinking_signature: model.thinking_signature,
            assistant_id: model.assistant_id,
            attachments,
            tool_use,
            created_at: model.created_at,
            updated_at: model.updated_at,
            source: model.source,
            error,
        }
    }

    /// Convert Message type to SeaORM ActiveModel
    fn message_to_active_model(message: &Message) -> Result<message::ActiveModel, DbError> {
        // Serialize structured types to JSON strings for DB storage
        let content_json = serde_json::to_string(&message.content).map_err(|e| {
            DbError::SerializationError(format!("Failed to serialize content: {}", e))
        })?;

        let tool_calls_json = to_json_option(&message.tool_calls).map_err(|e| {
            DbError::SerializationError(format!("Failed to serialize tool_calls: {}", e))
        })?;

        let attachments_json = to_json_option(&message.attachments).map_err(|e| {
            DbError::SerializationError(format!("Failed to serialize attachments: {}", e))
        })?;

        let tool_use_json = to_json_option(&message.tool_use).map_err(|e| {
            DbError::SerializationError(format!("Failed to serialize tool_use: {}", e))
        })?;

        let error_json = to_json_option(&message.error)
            .map_err(|e| DbError::SerializationError(format!("Failed to serialize error: {}", e)))?;

        Ok(message::ActiveModel {
            id: Set(message.id.clone()),
            session_id: Set(message.session_id.clone()),
            role: Set(message.role.clone()),
            content: Set(content_json),
            tool_calls: Set(tool_calls_json),
            tool_call_id: Set(message.tool_call_id.clone()),
            is_streaming: Set(message.is_streaming.map(|b| if b { 1 } else { 0 })),
            thinking: Set(message.thinking.clone()),
            thinking_signature: Set(message.thinking_signature.clone()),
            assistant_id: Set(message.assistant_id.clone()),
            attachments: Set(attachments_json),
            tool_use: Set(tool_use_json),
            created_at: Set(message.created_at),
            updated_at: Set(message.updated_at),
            source: Set(message.source.clone()),
            error: Set(error_json),
        })
    }
}

#[async_trait]
impl MessageRepository for SqliteMessageRepository {
    async fn create_table(&self) -> Result<(), DbError> {
        // No-op: Schema is now managed by SeaORM migrations
        log::debug!("create_table() called but schema is now managed by migrations");
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

        // Get total count using SeaORM
        let total = MessageEntity::find()
            .filter(message::Column::SessionId.eq(session_id))
            .count(&self.db)
            .await?;

        // Calculate offset
        let offset = (page.saturating_sub(1)) as u64 * page_size as u64;

        // Fetch paginated messages
        let models = MessageEntity::find()
            .filter(message::Column::SessionId.eq(session_id))
            .order_by_asc(message::Column::CreatedAt)
            .offset(offset)
            .limit(page_size as u64)
            .all(&self.db)
            .await?;

        let messages: Vec<Message> = models.into_iter().map(Self::model_to_message).collect();

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
        use sea_orm::sea_query::OnConflict;

        let model = Self::message_to_active_model(message)?;

        MessageEntity::insert(model)
            .on_conflict(
                OnConflict::column(message::Column::Id)
                    .update_columns([
                        message::Column::SessionId,
                        message::Column::Role,
                        message::Column::Content,
                        message::Column::ToolCalls,
                        message::Column::ToolCallId,
                        message::Column::IsStreaming,
                        message::Column::Thinking,
                        message::Column::ThinkingSignature,
                        message::Column::AssistantId,
                        message::Column::Attachments,
                        message::Column::ToolUse,
                        message::Column::UpdatedAt,
                        message::Column::Source,
                        message::Column::Error,
                    ])
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn insert_many(&self, messages: Vec<Message>) -> Result<(), DbError> {
        use sea_orm::sea_query::OnConflict;
        use sea_orm::TransactionTrait;

        let txn = self.db.begin().await?;

        for message in messages {
            let model = Self::message_to_active_model(&message)?;

            MessageEntity::insert(model)
                .on_conflict(
                    OnConflict::column(message::Column::Id)
                        .update_columns([
                            message::Column::SessionId,
                            message::Column::Role,
                            message::Column::Content,
                            message::Column::ToolCalls,
                            message::Column::ToolCallId,
                            message::Column::IsStreaming,
                            message::Column::Thinking,
                            message::Column::ThinkingSignature,
                            message::Column::AssistantId,
                            message::Column::Attachments,
                            message::Column::ToolUse,
                            message::Column::UpdatedAt,
                            message::Column::Source,
                            message::Column::Error,
                        ])
                        .to_owned(),
                )
                .exec(&txn)
                .await?;
        }

        txn.commit()
            .await
            .map_err(|e| DbError::TransactionFailed(e.to_string()))?;
        Ok(())
    }

    async fn delete_by_id(&self, message_id: &str) -> Result<(), DbError> {
        MessageEntity::delete_by_id(message_id)
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn delete_by_session(&self, session_id: &str) -> Result<(), DbError> {
        MessageEntity::delete_many()
            .filter(message::Column::SessionId.eq(session_id))
            .exec(&self.db)
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
        use sea_orm::sea_query::OnConflict;
        let now = chrono::Utc::now().timestamp_millis();

        let model = message_index_meta::ActiveModel {
            session_id: Set(session_id.to_string()),
            index_path: Set(Some(index_path.to_string())),
            last_indexed_at: Set(now),
            doc_count: Set(doc_count as i32),
            index_version: Set(1),
            last_rebuild_duration_ms: Set(Some(rebuild_duration_ms)),
        };

        MessageIndexMeta::insert(model)
            .on_conflict(
                OnConflict::column(message_index_meta::Column::SessionId)
                    .update_columns([
                        message_index_meta::Column::IndexPath,
                        message_index_meta::Column::LastIndexedAt,
                        message_index_meta::Column::DocCount,
                        message_index_meta::Column::LastRebuildDurationMs,
                    ])
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn get_last_indexed_at(&self, session_id: &str) -> Result<i64, DbError> {
        let result = MessageIndexMeta::find_by_id(session_id)
            .one(&self.db)
            .await?;

        Ok(result.map(|m| m.last_indexed_at).unwrap_or(0))
    }

    async fn is_index_dirty(&self, session_id: &str) -> Result<bool, DbError> {
        use sea_orm::sea_query::Expr;

        let last_indexed_at = self.get_last_indexed_at(session_id).await?;

        let max_created: Option<i64> = MessageEntity::find()
            .filter(message::Column::SessionId.eq(session_id))
            .select_only()
            .column_as(Expr::col(message::Column::CreatedAt).max(), "max_created")
            .into_tuple()
            .one(&self.db)
            .await?;

        Ok(max_created.map(|t| t > last_indexed_at).unwrap_or(false))
    }
}
