use thiserror::Error;

/// Database error types for repository operations
#[derive(Debug, Error)]
pub enum DbError {
    /// SeaORM database query execution failed
    #[error("SeaORM query failed: {0}")]
    SeaOrmQueryFailed(#[from] sea_orm::DbErr),

    /// Requested record was not found
    #[error("Record not found: {0}")]
    NotFound(String),

    /// Invalid input parameter provided
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Transaction commit or rollback failed
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    /// JSON serialization/deserialization failed
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Convert DbError to String for Tauri command compatibility
impl From<DbError> for String {
    fn from(err: DbError) -> String {
        err.to_string()
    }
}
