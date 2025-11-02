use super::error::DbError;
use async_trait::async_trait;
use sqlx::SqlitePool;

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
    pool: SqlitePool,
}

impl SqliteContentStoreRepository {
    /// Create a new SQLite content store repository with the given pool
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ContentStoreRepository for SqliteContentStoreRepository {
    async fn delete_by_session(&self, session_id: &str) -> Result<(), DbError> {
        // Delete chunks first (foreign key constraint)
        sqlx::query(
            "DELETE FROM chunks WHERE content_id IN (SELECT id FROM contents WHERE session_id = ?)",
        )
        .bind(session_id)
        .execute(&self.pool)
        .await?;

        // Delete contents
        sqlx::query("DELETE FROM contents WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        // Delete stores
        sqlx::query("DELETE FROM stores WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
