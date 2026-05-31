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
    pub to_id: String,
    pub condensed_count: Option<usize>,
    pub summary: String,
    pub created_at: i64,
}

impl From<compact_context::Model> for CompactContextRecord {
    fn from(model: compact_context::Model) -> Self {
        Self {
            id: model.id,
            session_id: model.session_id,
            to_id: model.to_id,
            condensed_count: model.condensed_count.map(|value| value as usize),
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
            to_id: Set(record.to_id.clone()),
            condensed_count: Set(record.condensed_count.map(|value| value as i32)),
            summary: Set(record.summary.clone()),
            created_at: Set(record.created_at),
        };

        CompactContextEntity::insert(active_model)
            .on_conflict(
                OnConflict::column(compact_context::Column::SessionId)
                    .update_columns([
                        compact_context::Column::ToId,
                        compact_context::Column::CondensedCount,
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
