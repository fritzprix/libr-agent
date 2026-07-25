use super::error::DbError;
use crate::entity::pending_queue;
use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingQueueEntry {
    pub message_id: String,
    pub session_id: String,
    pub created_at: i64,
}

#[async_trait]
pub trait PendingQueueRepository: Send + Sync {
    async fn enqueue(
        &self,
        session_id: &str,
        message_id: &str,
        created_at: i64,
    ) -> Result<(), DbError>;

    async fn list_by_session(&self, session_id: &str) -> Result<Vec<PendingQueueEntry>, DbError>;

    async fn remove(&self, message_id: &str) -> Result<(), DbError>;

    async fn remove_all_for_session(&self, session_id: &str) -> Result<Vec<String>, DbError>;
}

#[derive(Debug)]
pub struct SqlitePendingQueueRepository {
    db: DatabaseConnection,
}

impl SqlitePendingQueueRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PendingQueueRepository for SqlitePendingQueueRepository {
    async fn enqueue(
        &self,
        session_id: &str,
        message_id: &str,
        created_at: i64,
    ) -> Result<(), DbError> {
        let model = pending_queue::ActiveModel {
            message_id: Set(message_id.to_string()),
            session_id: Set(session_id.to_string()),
            created_at: Set(created_at),
        };
        model.insert(&self.db).await?;
        Ok(())
    }

    async fn list_by_session(&self, session_id: &str) -> Result<Vec<PendingQueueEntry>, DbError> {
        let rows = pending_queue::Entity::find()
            .filter(pending_queue::Column::SessionId.eq(session_id))
            .order_by_asc(pending_queue::Column::CreatedAt)
            .order_by_asc(pending_queue::Column::MessageId)
            .all(&self.db)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| PendingQueueEntry {
                message_id: row.message_id,
                session_id: row.session_id,
                created_at: row.created_at,
            })
            .collect())
    }

    async fn remove(&self, message_id: &str) -> Result<(), DbError> {
        pending_queue::Entity::delete_by_id(message_id.to_string())
            .exec(&self.db)
            .await?;
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
}
