use log::{info, warn};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, Statement};

/// Error type for schema validation failures
#[derive(Debug)]
pub enum SchemaValidationError {
    TableMissing(String),
    ColumnMissing { table: String, column: String },
    QueryFailed(DbErr),
}

impl std::fmt::Display for SchemaValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaValidationError::TableMissing(table) => {
                write!(f, "Table '{}' is missing from database", table)
            }
            SchemaValidationError::ColumnMissing { table, column } => {
                write!(f, "Column '{}' is missing from table '{}'", column, table)
            }
            SchemaValidationError::QueryFailed(err) => {
                write!(f, "Schema validation query failed: {}", err)
            }
        }
    }
}

impl std::error::Error for SchemaValidationError {}

/// Validates that all critical tables have the expected schema
///
/// This function checks:
/// - Core tables exist (sessions, messages, assistants)
/// - Planning module tables have correct columns
///
/// Returns Ok(()) if validation passes, or SchemaValidationError if any check fails
pub async fn validate_schema(db: &DatabaseConnection) -> Result<(), SchemaValidationError> {
    info!("🔍 Validating database schema...");

    // Validate core tables exist
    validate_table_exists(db, "sessions").await?;
    validate_table_exists(db, "messages").await?;
    validate_table_exists(db, "assistants").await?;

    // Validate sessions table has is_bookmarked and yolo_mode columns
    validate_table_columns(db, "sessions", &["id", "is_bookmarked", "yolo_mode"]).await?;

    // Validate planning module tables with specific columns
    validate_table_columns(
        db,
        "planning_goals",
        &["id", "session_id", "goal_text", "status", "created_at"],
    )
    .await?;

    validate_table_columns(
        db,
        "planning_todos",
        &[
            "id",
            "session_id",
            "content",
            "description",
            "priority",
            "is_checked", // Critical: completion tracking
            "status",
            "created_at",
            "updated_at",
        ],
    )
    .await?;

    validate_table_columns(
        db,
        "planning_scratchpad",
        &[
            "id",
            "session_id",
            "content",
            "title",
            "source",
            "tags",
            "created_at",
            "updated_at",
        ],
    )
    .await?;

    info!("✅ Database schema validation passed");
    Ok(())
}

/// Checks if a table exists in the database
async fn validate_table_exists(
    db: &DatabaseConnection,
    table_name: &str,
) -> Result<(), SchemaValidationError> {
    let exists = table_exists(db, table_name)
        .await
        .map_err(SchemaValidationError::QueryFailed)?;

    if !exists {
        warn!("❌ Table '{}' is missing", table_name);
        return Err(SchemaValidationError::TableMissing(table_name.to_string()));
    }

    Ok(())
}

/// Validates that a table has all required columns
async fn validate_table_columns(
    db: &DatabaseConnection,
    table_name: &str,
    required_columns: &[&str],
) -> Result<(), SchemaValidationError> {
    // First check if table exists
    validate_table_exists(db, table_name).await?;

    // Get actual columns from the table
    let actual_columns = get_table_columns(db, table_name)
        .await
        .map_err(SchemaValidationError::QueryFailed)?;

    // Check each required column
    for &required_col in required_columns {
        if !actual_columns.contains(&required_col.to_lowercase()) {
            warn!(
                "❌ Column '{}' is missing from table '{}'",
                required_col, table_name
            );
            return Err(SchemaValidationError::ColumnMissing {
                table: table_name.to_string(),
                column: required_col.to_string(),
            });
        }
    }

    Ok(())
}

/// Checks if a table exists in the database
async fn table_exists(db: &DatabaseConnection, table_name: &str) -> Result<bool, DbErr> {
    // Validate table name to prevent SQL injection (defense-in-depth)
    if !table_name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(DbErr::Custom(format!(
            "Invalid table name format: {}",
            table_name
        )));
    }

    let result = db
        .query_one(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT name FROM sqlite_master WHERE type='table' AND name=?",
            vec![table_name.into()],
        ))
        .await?;

    Ok(result.is_some())
}

/// Gets all column names for a table (lowercase)
async fn get_table_columns(
    db: &DatabaseConnection,
    table_name: &str,
) -> Result<Vec<String>, DbErr> {
    // Validate table name to prevent SQL injection since PRAGMA cannot be parameterized
    if !table_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(DbErr::Custom(format!("Invalid table name format: {}", table_name)));
    }

    let query = format!("PRAGMA table_info({})", table_name);

    let rows = db
        .query_all(Statement::from_string(db.get_database_backend(), query))
        .await?;

    let columns: Vec<String> = rows
        .iter()
        .filter_map(|row| {
            // Column name is at index 1 in PRAGMA table_info result
            row.try_get::<String>("", "name").ok()
        })
        .map(|name| name.to_lowercase())
        .collect();

    Ok(columns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    async fn setup_test_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        // Create test tables
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, name TEXT, is_bookmarked INTEGER, yolo_mode INTEGER)".to_string(),
        ))
        .await
        .expect("Failed to create sessions table");

        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TABLE messages (id TEXT PRIMARY KEY, content TEXT)".to_string(),
        ))
        .await
        .expect("Failed to create messages table");

        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TABLE assistants (id TEXT PRIMARY KEY, name TEXT)".to_string(),
        ))
        .await
        .expect("Failed to create assistants table");

        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TABLE planning_goals (id INTEGER PRIMARY KEY, session_id TEXT, goal_text TEXT, status TEXT, created_at INTEGER)".to_string(),
        ))
        .await
        .expect("Failed to create planning_goals table");

        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TABLE planning_todos (id INTEGER PRIMARY KEY, session_id TEXT, content TEXT, description TEXT, priority TEXT, is_checked INTEGER, status TEXT, created_at INTEGER, updated_at INTEGER)".to_string(),
        ))
        .await
        .expect("Failed to create planning_todos table");

        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TABLE planning_scratchpad (id INTEGER PRIMARY KEY, session_id TEXT, content TEXT, title TEXT, source TEXT, tags TEXT, created_at INTEGER, updated_at INTEGER)".to_string(),
        ))
        .await
        .expect("Failed to create planning_scratchpad table");

        db
    }

    #[tokio::test]
    async fn test_validate_schema_success() {
        let db = setup_test_db().await;
        let result = validate_schema(&db).await;
        assert!(result.is_ok(), "Schema validation failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_table_exists_invalid_name() {
        let db = setup_test_db().await;

        let injection_names = vec!["table; DROP TABLE sessions", "sessions' --", "sessions\"", " "];

        for name in injection_names {
            let result = table_exists(&db, name).await;
            assert!(
                result.is_err(),
                "table_exists should fail for invalid name: {}",
                name
            );
        }
    }

    #[tokio::test]
    async fn test_get_table_columns_invalid_name() {
        let db = setup_test_db().await;

        let injection_names = vec!["sessions; DROP TABLE sessions", "sessions' --", "sessions\"", " "];

        for name in injection_names {
            let result = get_table_columns(&db, name).await;
            assert!(
                result.is_err(),
                "get_table_columns should fail for invalid name: {}",
                name
            );
        }
    }

    #[tokio::test]
    async fn test_table_exists() {
        let db = setup_test_db().await;

        let exists = table_exists(&db, "sessions").await.unwrap();
        assert!(exists, "sessions table should exist");

        let not_exists = table_exists(&db, "nonexistent_table").await.unwrap();
        assert!(!not_exists, "nonexistent table should not exist");
    }

    #[tokio::test]
    async fn test_validate_table_missing() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        let result = validate_table_exists(&db, "sessions").await;
        assert!(matches!(
            result,
            Err(SchemaValidationError::TableMissing(_))
        ));
    }

    #[tokio::test]
    async fn test_validate_column_missing() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        // Create table without parent_id column
        db.execute(Statement::from_string(
            db.get_database_backend(),
            "CREATE TABLE planning_todos (id INTEGER PRIMARY KEY, session_id TEXT, content TEXT)"
                .to_string(),
        ))
        .await
        .expect("Failed to create table");

        let result = validate_table_columns(
            &db,
            "planning_todos",
            &["id", "session_id", "content", "priority"],
        )
        .await;

        assert!(matches!(
            result,
            Err(SchemaValidationError::ColumnMissing { .. })
        ));
    }

    #[tokio::test]
    async fn test_get_table_columns() {
        let db = setup_test_db().await;

        let columns = get_table_columns(&db, "planning_todos").await.unwrap();

        assert!(columns.contains(&"id".to_string()));
        assert!(columns.contains(&"is_checked".to_string()));
    }
}
