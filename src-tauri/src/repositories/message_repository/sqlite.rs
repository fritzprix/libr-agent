use async_trait::async_trait;
use sea_orm::{
    sea_query::Expr, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Statement,
};

use crate::entity::prelude::Message as MessageEntity;
use crate::entity::message;
use crate::models::chat::Message;
use crate::utils::pagination::Page;

use super::super::error::DbError;
use super::index_meta;
use super::persist;
use super::types::{MessageRepository, MessageRowWithCursor, MessageSlicePage};

/// SQLite implementation of MessageRepository using SeaORM
#[derive(Debug)]
pub struct SqliteMessageRepository {
    pub(crate) db: DatabaseConnection,
}

impl SqliteMessageRepository {
    /// Create a new SQLite message repository with the given database connection
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn query_slice_rows(
        &self,
        sql: &str,
        values: Vec<sea_orm::Value>,
    ) -> Result<Vec<MessageRowWithCursor>, DbError> {
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                values,
            ))
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        rows.iter()
            .map(persist::row_to_message_with_cursor)
            .collect::<Result<Vec<_>, _>>()
    }

    async fn query_message_models(
        &self,
        sql: &str,
        values: Vec<sea_orm::Value>,
    ) -> Result<Vec<message::Model>, DbError> {
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                sql,
                values,
            ))
            .await
            .map_err(DbError::SeaOrmQueryFailed)?;

        rows.iter()
            .map(persist::row_to_message_model)
            .collect::<Result<Vec<_>, _>>()
    }

    /// Stable `pub(crate)` surface used by pending-queue compaction.
    pub(crate) fn message_to_active_model(
        message: &Message,
    ) -> Result<message::ActiveModel, DbError> {
        persist::message_to_active_model(message)
    }

    pub(crate) fn get_upsert_on_conflict() -> sea_orm::sea_query::OnConflict {
        persist::get_upsert_on_conflict()
    }

    pub(crate) async fn update_session_last_message_at<C>(
        db: &C,
        session_id: &str,
        last_message_at: i64,
    ) -> Result<(), DbError>
    where
        C: sea_orm::ConnectionTrait,
    {
        persist::update_session_last_message_at(db, session_id, last_message_at).await
    }
}

#[async_trait]
impl MessageRepository for SqliteMessageRepository {
    async fn get_page(
        &self,
        session_id: &str,
        page: u64,
        page_size: u64,
    ) -> Result<Page<Message>, DbError> {
        if page_size == 0 {
            return Err(DbError::InvalidInput("page_size must be > 0".into()));
        }

        let total = MessageEntity::find()
            .filter(message::Column::SessionId.eq(session_id))
            .count(&self.db)
            .await?;

        let offset = page.saturating_sub(1).saturating_mul(page_size);

        // Fetch paginated messages in persisted causal order. Using rowid avoids treating
        // cross-layer created_at skew as conversation truth.
        let models = self
            .query_message_models(
                "SELECT id, session_id, role, content, tool_calls, tool_call_id, is_streaming, thinking, thinking_signature, assistant_id, attachments, tool_use, created_at, updated_at, source, error, usage, prompt_tokens \
                 FROM messages \
                 WHERE session_id = ? \
                 ORDER BY rowid ASC \
                 LIMIT ? OFFSET ?",
                vec![
                    session_id.into(),
                    (page_size as i64).into(),
                    (offset as i64).into(),
                ],
            )
            .await?;

        let messages: Vec<Message> = models.into_iter().map(persist::model_to_message).collect();

        Ok(Page::new(messages, page, page_size, total))
    }

    async fn insert(&self, message: &Message) -> Result<(), DbError> {
        let model = persist::message_to_active_model(message)?;

        MessageEntity::insert(model)
            .on_conflict(persist::get_upsert_on_conflict())
            .exec(&self.db)
            .await?;

        persist::update_session_last_message_at(&self.db, &message.session_id, message.created_at)
            .await?;

        Ok(())
    }

    async fn insert_many(&self, messages: Vec<Message>) -> Result<(), DbError> {
        use sea_orm::TransactionTrait;
        use std::collections::HashMap;

        let txn = self.db.begin().await?;
        let mut latest_by_session: HashMap<String, i64> = HashMap::new();

        for message in messages {
            let model = persist::message_to_active_model(&message)?;

            MessageEntity::insert(model)
                .on_conflict(persist::get_upsert_on_conflict())
                .exec(&txn)
                .await?;

            latest_by_session
                .entry(message.session_id.clone())
                .and_modify(|current| *current = (*current).max(message.created_at))
                .or_insert(message.created_at);
        }

        for (session_id, last_message_at) in latest_by_session {
            persist::update_session_last_message_at(&txn, &session_id, last_message_at).await?;
        }

        txn.commit()
            .await
            .map_err(|e| DbError::TransactionFailed(e.to_string()))?;
        Ok(())
    }

    async fn get_by_id(&self, message_id: &str) -> Result<Option<Message>, DbError> {
        let model = MessageEntity::find_by_id(message_id).one(&self.db).await?;
        Ok(model.map(persist::model_to_message))
    }

    async fn get_by_ids(&self, message_ids: Vec<String>) -> Result<Vec<Message>, DbError> {
        if message_ids.is_empty() {
            return Ok(Vec::new());
        }

        let models = MessageEntity::find()
            .filter(message::Column::Id.is_in(message_ids.clone()))
            .all(&self.db)
            .await?;

        let mut msg_map: std::collections::HashMap<String, Message> = models
            .into_iter()
            .map(|m| (m.id.clone(), persist::model_to_message(m)))
            .collect();

        let result = message_ids
            .into_iter()
            .filter_map(|id| msg_map.remove(&id))
            .collect();

        Ok(result)
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
        index_meta::update_index_meta(
            &self.db,
            session_id,
            index_path,
            doc_count,
            rebuild_duration_ms,
        )
        .await
    }

    async fn get_last_indexed_at(&self, session_id: &str) -> Result<i64, DbError> {
        index_meta::get_last_indexed_at(&self.db, session_id).await
    }

    async fn is_index_dirty(&self, session_id: &str) -> Result<bool, DbError> {
        index_meta::is_index_dirty(&self.db, session_id).await
    }

    async fn delete_index_metadata(&self, session_id: &str) -> Result<(), DbError> {
        index_meta::delete_index_metadata(&self.db, session_id).await
    }

    async fn get_messages_by_session(
        &self,
        session_id: &str,
        limit: u64,
    ) -> Result<Vec<Message>, DbError> {
        let models = self
            .get_message_models_by_session(session_id, limit)
            .await?;
        Ok(models.into_iter().map(persist::model_to_message).collect())
    }

    async fn get_recent_slice(
        &self,
        session_id: &str,
        limit: u64,
    ) -> Result<MessageSlicePage, DbError> {
        let fetch_limit = persist::validate_slice_limit(limit)?;
        let rows = self
            .query_slice_rows(
                "SELECT rowid AS cursor_rowid, id, session_id, role, content, tool_calls, tool_call_id, is_streaming, thinking, thinking_signature, assistant_id, attachments, tool_use, created_at, updated_at, source, error, usage, prompt_tokens \
                 FROM messages \
                 WHERE session_id = ? \
                 ORDER BY rowid DESC \
                 LIMIT ?",
                vec![session_id.into(), fetch_limit.into()],
            )
            .await?;

        persist::build_slice_page(rows, limit)
    }

    async fn get_messages_before(
        &self,
        session_id: &str,
        before_row_id: i64,
        limit: u64,
    ) -> Result<MessageSlicePage, DbError> {
        let fetch_limit = persist::validate_slice_limit(limit)?;
        let rows = self
            .query_slice_rows(
                "SELECT rowid AS cursor_rowid, id, session_id, role, content, tool_calls, tool_call_id, is_streaming, thinking, thinking_signature, assistant_id, attachments, tool_use, created_at, updated_at, source, error, usage, prompt_tokens \
                 FROM messages \
                 WHERE session_id = ? \
                   AND rowid < ? \
                 ORDER BY rowid DESC \
                 LIMIT ?",
                vec![
                    session_id.into(),
                    before_row_id.into(),
                    fetch_limit.into(),
                ],
            )
            .await?;

        persist::build_slice_page(rows, limit)
    }

    async fn get_recent_messages(&self, limit: u64) -> Result<Vec<Message>, DbError> {
        let models = self.get_recent_message_models(limit).await?;
        Ok(models.into_iter().map(persist::model_to_message).collect())
    }

    async fn get_distinct_sessions(&self) -> Result<Vec<String>, DbError> {
        let sessions: Vec<String> = MessageEntity::find()
            .select_only()
            .column(message::Column::SessionId)
            .distinct()
            .into_tuple()
            .all(&self.db)
            .await?;

        Ok(sessions)
    }

    async fn count_by_session(&self) -> Result<Vec<(String, u64)>, DbError> {
        let rows: Vec<(String, i64)> = MessageEntity::find()
            .select_only()
            .column(message::Column::SessionId)
            .column_as(Expr::col(message::Column::Id).count(), "message_count")
            .group_by(message::Column::SessionId)
            .into_tuple()
            .all(&self.db)
            .await?;

        Ok(rows
            .into_iter()
            .map(|(session_id, count)| (session_id, count.max(0) as u64))
            .collect())
    }

    async fn get_message_models_by_session(
        &self,
        session_id: &str,
        limit: u64,
    ) -> Result<Vec<message::Model>, DbError> {
        let models = self
            .query_message_models(
                "SELECT id, session_id, role, content, tool_calls, tool_call_id, is_streaming, thinking, thinking_signature, assistant_id, attachments, tool_use, created_at, updated_at, source, error, usage, prompt_tokens \
                 FROM messages \
                 WHERE session_id = ? \
                 ORDER BY rowid DESC \
                 LIMIT ?",
                vec![session_id.into(), (limit as i64).into()],
            )
            .await?;

        Ok(models)
    }

    async fn get_recent_message_models(&self, limit: u64) -> Result<Vec<message::Model>, DbError> {
        let models = self
            .query_message_models(
                "SELECT id, session_id, role, content, tool_calls, tool_call_id, is_streaming, thinking, thinking_signature, assistant_id, attachments, tool_use, created_at, updated_at, source, error, usage, prompt_tokens \
                 FROM messages \
                 ORDER BY rowid DESC \
                 LIMIT ?",
                vec![(limit as i64).into()],
            )
            .await?;

        Ok(models)
    }
}
