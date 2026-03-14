use sea_orm::DbErr;
use std::io;

/// Database initialization and migration errors
#[derive(Debug)]
pub enum DatabaseError {
    /// Failed to connect to database
    ConnectionFailed(String),

    /// Migration execution failed
    MigrationFailed {
        migration: String,
        error: String,
        backup_path: Option<String>,
    },

    /// Schema validation failed (recoverable)
    SchemaValidationFailed { issues: Vec<SchemaIssue> },

    /// Database file is locked by another process
    FileLocked { path: String, attempts: u32 },

    /// Failed to create backup
    BackupFailed { path: String, error: String },

    /// Failed to restore from backup
    RestoreFailed { backup_path: String, error: String },

    /// Database file corruption detected
    CorruptedDatabase {
        path: String,
        details: String,
        backup_available: bool,
    },

    /// Migration file was modified after being applied
    MigrationModified {
        migration: String,
        expected_hash: String,
        found_hash: String,
    },

    /// `SeaORM` error
    SeaOrmError(DbErr),

    /// IO error
    IoError(io::Error),
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseError::ConnectionFailed(msg) => {
                write!(f, "Failed to connect to database: {}", msg)
            }
            DatabaseError::MigrationFailed {
                migration,
                error,
                backup_path,
            } => {
                if let Some(backup) = backup_path {
                    write!(
                        f,
                        "Migration '{}' failed: {}. Backup available at: {}",
                        migration, error, backup
                    )
                } else {
                    write!(f, "Migration '{}' failed: {}", migration, error)
                }
            }
            DatabaseError::SchemaValidationFailed { issues } => {
                write!(f, "Schema validation failed with {} issues", issues.len())
            }
            DatabaseError::FileLocked { path, attempts } => {
                write!(
                    f,
                    "Database file is locked (tried {} times): {}",
                    attempts, path
                )
            }
            DatabaseError::BackupFailed { path, error } => {
                write!(f, "Failed to create backup of {}: {}", path, error)
            }
            DatabaseError::RestoreFailed { backup_path, error } => {
                write!(f, "Failed to restore from {}: {}", backup_path, error)
            }
            DatabaseError::CorruptedDatabase {
                path,
                details,
                backup_available,
            } => {
                if *backup_available {
                    write!(
                        f,
                        "Database corrupted at {}: {}. Backup available for recovery.",
                        path, details
                    )
                } else {
                    write!(
                        f,
                        "Database corrupted at {}: {}. No backup available.",
                        path, details
                    )
                }
            }
            DatabaseError::MigrationModified {
                migration,
                expected_hash,
                found_hash,
            } => {
                write!(
                    f,
                    "Migration '{}' was modified after being applied! Expected: {}, Found: {}",
                    migration, expected_hash, found_hash
                )
            }
            DatabaseError::SeaOrmError(e) => write!(f, "Database error: {}", e),
            DatabaseError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for DatabaseError {}

impl From<DbErr> for DatabaseError {
    fn from(err: DbErr) -> Self {
        DatabaseError::SeaOrmError(err)
    }
}

impl From<io::Error> for DatabaseError {
    fn from(err: io::Error) -> Self {
        DatabaseError::IoError(err)
    }
}

/// Schema validation issue
#[derive(Debug, Clone)]
pub struct SchemaIssue {
    pub severity: SchemaSeverity,
    pub table: String,
    pub column: Option<String>,
    pub expected: String,
    pub found: String,
    pub fix_suggestion: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SchemaSeverity {
    Critical, // App cannot function
    Warning,  // Some features may not work
    Info,     // Optional features
}

impl SchemaIssue {
    pub fn critical(
        table: impl Into<String>,
        expected: impl Into<String>,
        found: impl Into<String>,
    ) -> Self {
        Self {
            severity: SchemaSeverity::Critical,
            table: table.into(),
            column: None,
            expected: expected.into(),
            found: found.into(),
            fix_suggestion: None,
        }
    }

    pub fn warning(
        table: impl Into<String>,
        column: impl Into<String>,
        issue: impl Into<String>,
    ) -> Self {
        Self {
            severity: SchemaSeverity::Warning,
            table: table.into(),
            column: Some(column.into()),
            expected: issue.into(),
            found: "".into(),
            fix_suggestion: None,
        }
    }
}

/// Result type for database operations
pub type DatabaseResult<T> = Result<T, DatabaseError>;
