use super::error::DbError;
use async_trait::async_trait;
use sqlx::SqlitePool;

/// Session repository trait for abstraction and testability
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Delete index metadata for a specific session
    async fn delete_index_metadata(&self, session_id: &str) -> Result<(), DbError>;
}

/// SQLite implementation of SessionRepository
#[derive(Debug)]
pub struct SqliteSessionRepository {
    pool: SqlitePool,
}

impl SqliteSessionRepository {
    /// Create a new SQLite session repository with the given pool
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for SqliteSessionRepository {
    async fn delete_index_metadata(&self, session_id: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM message_index_meta WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
