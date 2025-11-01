use thiserror::Error;

/// Database error types for repository operations
#[derive(Debug, Error)]
pub enum DbError {
    /// Database query execution failed
    #[error("Database query failed: {0}")]
    QueryFailed(#[from] sqlx::Error),

    /// Requested record was not found
    #[error("Record not found: {0}")]
    NotFound(String),

    /// Invalid input parameter provided
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Transaction commit or rollback failed
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),
}

/// Convert DbError to String for Tauri command compatibility
impl From<DbError> for String {
    fn from(err: DbError) -> String {
        err.to_string()
    }
}
