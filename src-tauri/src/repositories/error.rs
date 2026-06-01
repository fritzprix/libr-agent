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

    /// Resource already exists
    #[error("Resource already exists: {0}")]
    DuplicateResource(String),

    /// Resource not found
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),
}

impl DbError {
    pub fn is_sqlite_busy(&self) -> bool {
        match self {
            DbError::SeaOrmQueryFailed(sea_orm::DbErr::Conn(sea_orm::RuntimeErr::SqlxError(
                err,
            )))
            | DbError::SeaOrmQueryFailed(sea_orm::DbErr::Exec(sea_orm::RuntimeErr::SqlxError(
                err,
            )))
            | DbError::SeaOrmQueryFailed(sea_orm::DbErr::Query(sea_orm::RuntimeErr::SqlxError(
                err,
            ))) => match err {
                sea_orm::SqlxError::Database(db_err) => {
                    let code = db_err.code().map(|code| code.into_owned());
                    let message = db_err.message().to_ascii_lowercase();

                    matches!(
                        code.as_deref(),
                        Some("5" | "6" | "SQLITE_BUSY" | "SQLITE_LOCKED")
                    ) || message.contains("database is locked")
                        || message.contains("database table is locked")
                }
                _ => false,
            },
            _ => false,
        }
    }
}

/// Convert DbError to String for Tauri command compatibility
impl From<DbError> for String {
    fn from(err: DbError) -> String {
        err.to_string()
    }
}
