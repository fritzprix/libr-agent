use super::error::DbError;
use crate::entity::{chunk, content, store};
use async_trait::async_trait;
use sea_orm::{
    sea_query::{Expr, Query},
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};

/// Content store repository trait for abstraction and testability
#[async_trait]
pub trait ContentStoreRepository: Send + Sync {
    /// Delete all content store data for a specific session
    /// This includes chunks, contents, and stores tables
    async fn delete_by_session(&self, session_id: &str) -> Result<(), DbError>;
}

/// SQLite implementation of ContentStoreRepository
#[derive(Debug)]
pub struct SqliteContentStoreRepository {
    db: DatabaseConnection,
}

impl SqliteContentStoreRepository {
    /// Create a new SQLite content store repository with the given database connection
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ContentStoreRepository for SqliteContentStoreRepository {
    async fn delete_by_session(&self, session_id: &str) -> Result<(), DbError> {
        // Delete chunks first (foreign key constraint)
        // DELETE FROM chunks WHERE content_id IN (SELECT id FROM contents WHERE session_id = ?)
        chunk::Entity::delete_many()
            .filter(
                chunk::Column::ContentId.in_subquery(
                    Query::select()
                        .column(content::Column::Id)
                        .from(content::Entity)
                        .and_where(Expr::col(content::Column::SessionId).eq(session_id))
                        .to_owned(),
                ),
            )
            .exec(&self.db)
            .await?;

        // Delete contents
        // DELETE FROM contents WHERE session_id = ?
        content::Entity::delete_many()
            .filter(content::Column::SessionId.eq(session_id))
            .exec(&self.db)
            .await?;

        // Delete stores
        // DELETE FROM stores WHERE session_id = ?
        store::Entity::delete_many()
            .filter(store::Column::SessionId.eq(session_id))
            .exec(&self.db)
            .await?;

        Ok(())
    }
}
