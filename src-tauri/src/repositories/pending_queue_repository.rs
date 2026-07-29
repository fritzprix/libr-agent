use super::error::DbError;
use crate::entity::pending_queue;
use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set, Statement, TransactionTrait,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingQueueEntry {
    pub message_id: String,
    pub session_id: String,
    pub created_at: i64,
    pub queue_seq: i64,
}

#[async_trait]
pub trait PendingQueueRepository: Send + Sync {
    /// Allocate the next monotonic queue_seq and insert.
    async fn enqueue(
        &self,
        session_id: &str,
        message_id: &str,
        created_at: i64,
    ) -> Result<PendingQueueEntry, DbError>;

    /// Re-insert with an existing queue_seq (failure recovery / restore).
    async fn enqueue_with_seq(
        &self,
        session_id: &str,
        message_id: &str,
        created_at: i64,
        queue_seq: i64,
    ) -> Result<(), DbError>;

    async fn list_by_session(&self, session_id: &str) -> Result<Vec<PendingQueueEntry>, DbError>;

    async fn get(&self, message_id: &str) -> Result<Option<PendingQueueEntry>, DbError>;

    /// Remove index row and return it (for claim/cancel that must preserve seq).
    async fn remove_returning(
        &self,
        message_id: &str,
    ) -> Result<Option<PendingQueueEntry>, DbError>;

    async fn remove(&self, message_id: &str) -> Result<(), DbError>;

    /// Atomically drop the index row and the backing messages row.
    async fn remove_index_and_message(
        &self,
        message_id: &str,
    ) -> Result<Option<PendingQueueEntry>, DbError>;

    /// Atomically upsert the merged keeper message, delete absorbed message rows
    /// (cascade-clears their pending_queue rows), and remove the keeper index row.
    async fn commit_merged_claim(
        &self,
        keeper: &crate::models::chat::Message,
        absorbed_message_ids: &[String],
    ) -> Result<(), DbError>;

    async fn remove_all_for_session(&self, session_id: &str) -> Result<Vec<String>, DbError>;

    /// Drop index rows whose message no longer exists.
    async fn delete_orphans_for_session(&self, session_id: &str) -> Result<u64, DbError>;
}

#[derive(Debug)]
pub struct SqlitePendingQueueRepository {
    db: DatabaseConnection,
}

impl SqlitePendingQueueRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn next_queue_seq(db: &impl ConnectionTrait) -> Result<i64, DbError> {
        let result = db
            .query_one(Statement::from_string(
                db.get_database_backend(),
                "SELECT COALESCE(MAX(queue_seq), 0) AS max_seq FROM pending_queue".to_string(),
            ))
            .await?;

        let max_seq = match result {
            Some(row) => row.try_get::<i64>("", "max_seq").unwrap_or(0),
            None => 0,
        };
        Ok(max_seq + 1)
    }

    fn model_to_entry(model: pending_queue::Model) -> PendingQueueEntry {
        PendingQueueEntry {
            message_id: model.message_id,
            session_id: model.session_id,
            created_at: model.created_at,
            queue_seq: model.queue_seq,
        }
    }
}

#[async_trait]
impl PendingQueueRepository for SqlitePendingQueueRepository {
    async fn enqueue(
        &self,
        session_id: &str,
        message_id: &str,
        created_at: i64,
    ) -> Result<PendingQueueEntry, DbError> {
        let queue_seq = Self::next_queue_seq(&self.db).await?;
        let model = pending_queue::ActiveModel {
            message_id: Set(message_id.to_string()),
            session_id: Set(session_id.to_string()),
            created_at: Set(created_at),
            queue_seq: Set(queue_seq),
        };
        let inserted = model.insert(&self.db).await?;
        Ok(Self::model_to_entry(inserted))
    }

    async fn enqueue_with_seq(
        &self,
        session_id: &str,
        message_id: &str,
        created_at: i64,
        queue_seq: i64,
    ) -> Result<(), DbError> {
        let model = pending_queue::ActiveModel {
            message_id: Set(message_id.to_string()),
            session_id: Set(session_id.to_string()),
            created_at: Set(created_at),
            queue_seq: Set(queue_seq),
        };
        model.insert(&self.db).await?;
        Ok(())
    }

    async fn list_by_session(&self, session_id: &str) -> Result<Vec<PendingQueueEntry>, DbError> {
        let rows = pending_queue::Entity::find()
            .filter(pending_queue::Column::SessionId.eq(session_id))
            .order_by_asc(pending_queue::Column::QueueSeq)
            .all(&self.db)
            .await?;

        Ok(rows.into_iter().map(Self::model_to_entry).collect())
    }

    async fn get(&self, message_id: &str) -> Result<Option<PendingQueueEntry>, DbError> {
        let row = pending_queue::Entity::find_by_id(message_id.to_string())
            .one(&self.db)
            .await?;
        Ok(row.map(Self::model_to_entry))
    }

    async fn remove_returning(
        &self,
        message_id: &str,
    ) -> Result<Option<PendingQueueEntry>, DbError> {
        let entry = self.get(message_id).await?;
        if entry.is_some() {
            pending_queue::Entity::delete_by_id(message_id.to_string())
                .exec(&self.db)
                .await?;
        }
        Ok(entry)
    }

    async fn remove(&self, message_id: &str) -> Result<(), DbError> {
        pending_queue::Entity::delete_by_id(message_id.to_string())
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn remove_index_and_message(
        &self,
        message_id: &str,
    ) -> Result<Option<PendingQueueEntry>, DbError> {
        let txn = self.db.begin().await?;
        let row = pending_queue::Entity::find_by_id(message_id.to_string())
            .one(&txn)
            .await?;
        let Some(model) = row else {
            txn.commit().await?;
            return Ok(None);
        };
        let entry = Self::model_to_entry(model);
        pending_queue::Entity::delete_by_id(message_id.to_string())
            .exec(&txn)
            .await?;
        crate::entity::message::Entity::delete_by_id(message_id.to_string())
            .exec(&txn)
            .await?;
        txn.commit().await?;
        Ok(Some(entry))
    }

    async fn commit_merged_claim(
        &self,
        keeper: &crate::models::chat::Message,
        absorbed_message_ids: &[String],
    ) -> Result<(), DbError> {
        use crate::entity::message;
        use crate::entity::prelude::Message as MessageEntity;
        use crate::repositories::message_repository::SqliteMessageRepository;
        use sea_orm::EntityTrait;

        let txn = self.db.begin().await?;

        let active_model = SqliteMessageRepository::message_to_active_model(keeper)?;
        crate::entity::message::Entity::insert(active_model)
            .on_conflict(SqliteMessageRepository::get_upsert_on_conflict())
            .exec(&txn)
            .await?;

        if !absorbed_message_ids.is_empty() {
            crate::entity::message::Entity::delete_many()
                .filter(
                    crate::entity::message::Column::Id.is_in(absorbed_message_ids.iter().cloned()),
                )
                .exec(&txn)
                .await?;
        }

        SqliteMessageRepository::update_session_last_message_at(
            &txn,
            &keeper.session_id,
            keeper.created_at,
        )
        .await?;

        let keeper_model = SqliteMessageRepository::message_to_active_model(keeper)?;
        MessageEntity::insert(keeper_model)
            .on_conflict(SqliteMessageRepository::get_upsert_on_conflict())
            .exec(&txn)
            .await?;

        if !absorbed_message_ids.is_empty() {
            MessageEntity::delete_many()
                .filter(message::Column::Id.is_in(absorbed_message_ids.iter().cloned()))
                .exec(&txn)
                .await?;
        }

        let mut all_claimed_ids = vec![keeper.id.clone()];
        all_claimed_ids.extend(absorbed_message_ids.iter().cloned());

        pending_queue::Entity::delete_many()
            .filter(pending_queue::Column::MessageId.is_in(all_claimed_ids))
            .exec(&txn)
            .await?;

        txn.commit().await?;
        Ok(())
    }

    async fn remove_all_for_session(&self, session_id: &str) -> Result<Vec<String>, DbError> {
        let entries = self.list_by_session(session_id).await?;
        let ids: Vec<String> = entries.into_iter().map(|e| e.message_id).collect();
        if !ids.is_empty() {
            pending_queue::Entity::delete_many()
                .filter(pending_queue::Column::SessionId.eq(session_id))
                .exec(&self.db)
                .await?;
        }
        Ok(ids)
    }

    async fn delete_orphans_for_session(&self, session_id: &str) -> Result<u64, DbError> {
        let result = self
            .db
            .execute(Statement::from_sql_and_values(
                self.db.get_database_backend(),
                r#"
                DELETE FROM pending_queue
                WHERE session_id = ?
                  AND message_id NOT IN (SELECT id FROM messages)
                "#,
                [session_id.into()],
            ))
            .await?;
        Ok(result.rows_affected())
    }
}
