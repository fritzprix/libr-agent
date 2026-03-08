use super::error::DbError;
use crate::entity::compact_context;
use crate::entity::prelude::CompactContext as CompactContextEntity;
use async_trait::async_trait;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactContextRecord {
    pub id: String,
    pub session_id: String,
    pub from_id: String,
    pub to_id: String,
    pub summary: String,
    pub created_at: i64,
}

impl From<compact_context::Model> for CompactContextRecord {
    fn from(model: compact_context::Model) -> Self {
        Self {
            id: model.id,
            session_id: model.session_id,
            from_id: model.from_id,
            to_id: model.to_id,
            summary: model.summary,
            created_at: model.created_at,
        }
    }
}

#[async_trait]
pub trait CompactContextRepository: Send + Sync {
    async fn upsert(&self, record: &CompactContextRecord) -> Result<(), DbError>;
    async fn get_by_session_id(
        &self,
        session_id: &str,
    ) -> Result<Option<CompactContextRecord>, DbError>;
    async fn delete_by_session_id(&self, session_id: &str) -> Result<(), DbError>;
}

#[derive(Clone, Debug)]
pub struct SqliteCompactContextRepository {
    db: DatabaseConnection,
}

impl SqliteCompactContextRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CompactContextRepository for SqliteCompactContextRepository {
    async fn upsert(&self, record: &CompactContextRecord) -> Result<(), DbError> {
        use sea_orm::sea_query::OnConflict;

        let active_model = compact_context::ActiveModel {
            id: Set(record.id.clone()),
            session_id: Set(record.session_id.clone()),
            from_id: Set(record.from_id.clone()),
            to_id: Set(record.to_id.clone()),
            summary: Set(record.summary.clone()),
            created_at: Set(record.created_at),
        };

        CompactContextEntity::insert(active_model)
            .on_conflict(
                OnConflict::column(compact_context::Column::SessionId)
                    .update_columns([
                        compact_context::Column::FromId,
                        compact_context::Column::ToId,
                        compact_context::Column::Summary,
                        compact_context::Column::CreatedAt,
                    ])
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn get_by_session_id(
        &self,
        session_id: &str,
    ) -> Result<Option<CompactContextRecord>, DbError> {
        let model = CompactContextEntity::find()
            .filter(compact_context::Column::SessionId.eq(session_id))
            .one(&self.db)
            .await?;

        Ok(model.map(Into::into))
    }

    async fn delete_by_session_id(&self, session_id: &str) -> Result<(), DbError> {
        CompactContextEntity::delete_many()
            .filter(compact_context::Column::SessionId.eq(session_id))
            .exec(&self.db)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    async fn setup_test_db() -> SqliteCompactContextRepository {
        let db = Database::connect("sqlite::memory:").await.unwrap();

        // Run migrations
        use migration::{Migrator, MigratorTrait};
        Migrator::up(&db, None).await.unwrap();

        SqliteCompactContextRepository::new(db)
    }

    #[tokio::test]
    async fn test_compact_context_crud() {
        let repo = setup_test_db().await;
        let db = &repo.db;

        // Create a parent session first to avoid foreign key violation
        use crate::entity::session;
        let session = session::ActiveModel {
            id: Set("session-1".to_string()),
            status: Set("Idle".to_string()),
            model: Set("gpt-4".to_string()),
            provider: Set("openai".to_string()),
            created_at: Set(123456789),
            updated_at: Set(123456789),
            is_bookmarked: Set(false),
            yolo_mode: Set(false),
            ..Default::default()
        };
        session::Entity::insert(session).exec(db).await.unwrap();

        let record = CompactContextRecord {
            id: "cc-1".to_string(),
            session_id: "session-1".to_string(),
            from_id: "msg-1".to_string(),
            to_id: "msg-10".to_string(),
            summary: "Test summary".to_string(),
            created_at: 123456789,
        };

        // Test upsert
        repo.upsert(&record).await.unwrap();

        // Test get
        let retrieved = repo.get_by_session_id("session-1").await.unwrap().unwrap();
        assert_eq!(retrieved.summary, "Test summary");
        assert_eq!(retrieved.from_id, "msg-1");

        // Test update (upsert conflict)
        let mut updated = record.clone();
        updated.summary = "Updated summary".to_string();
        repo.upsert(&updated).await.unwrap();

        let retrieved = repo.get_by_session_id("session-1").await.unwrap().unwrap();
        assert_eq!(retrieved.summary, "Updated summary");

        // Test delete
        repo.delete_by_session_id("session-1").await.unwrap();
        let retrieved = repo.get_by_session_id("session-1").await.unwrap();
        assert!(retrieved.is_none());
    }
}
