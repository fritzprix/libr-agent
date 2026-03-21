//! Migration helper utilities
//!
//! Shared patterns for safe, idempotent migration operations.
//! Use these instead of raw SQL strings to avoid common pitfalls:
//! - `COUNT(*)` column aliasing for SeaORM compatibility
//! - Table/column existence checks
//! - Idempotent inserts

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

/// Count rows in a table.
///
/// Returns `None` if the query returns no rows (shouldn't happen for COUNT),
/// or `Some(n)` with the row count.
///
/// # Why not `COUNT(*)`?
/// SeaORM's `try_get("", "COUNT(*)")` is unreliable across backends.
/// This helper always aliases the column to `row_count` for safe extraction.
pub async fn count_rows(
    manager: &SchemaManager<'_>,
    table: impl IntoTableRef,
) -> Result<i64, DbErr> {
    let count_query = Query::select()
        .expr_as(Expr::cust("COUNT(*)"), Alias::new("row_count"))
        .from(table)
        .to_owned();

    let result = manager
        .get_connection()
        .query_one(manager.get_database_backend().build(&count_query))
        .await?;

    match result {
        Some(row) => row
            .try_get("", "row_count")
            .map_err(|e| DbErr::Custom(format!("Failed to read row_count: {e}"))),
        // COUNT(*) always returns exactly one row; if somehow None, treat as 0.
        None => Ok(0),
    }
}

/// Check whether a table exists in the SQLite schema.
pub async fn table_exists(manager: &SchemaManager<'_>, table_name: &str) -> Result<bool, DbErr> {
    let result = manager
        .get_connection()
        .query_one(Statement::from_sql_and_values(
            manager.get_database_backend(),
            "SELECT name FROM sqlite_master WHERE type='table' AND name=?",
            [table_name.into()],
        ))
        .await?;

    Ok(result.is_some())
}

/// Check whether a column exists in a SQLite table via PRAGMA table_info.
pub async fn column_exists(
    manager: &SchemaManager<'_>,
    table_name: &str,
    column_name: &str,
) -> Result<bool, DbErr> {
    // Validate table name to prevent SQL injection since PRAGMA cannot be parameterized
    if table_name.is_empty()
        || !table_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(DbErr::Custom(format!(
            "Invalid table name format: {:?}",
            table_name
        )));
    }

    let rows = manager
        .get_connection()
        .query_all(Statement::from_string(
            manager.get_database_backend(),
            format!("PRAGMA table_info({table_name})"),
        ))
        .await?;

    let col_lower = column_name.to_lowercase();
    for row in &rows {
        let name: String = row
            .try_get("", "name")
            .map_err(|e| DbErr::Custom(format!("PRAGMA table_info parse error: {e}")))?;
        if name.to_lowercase() == col_lower {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::Database;

    #[tokio::test]
    async fn test_column_exists_validation() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let manager = SchemaManager::new(&db);

        // Valid name
        let result = column_exists(&manager, "valid_table", "col").await;
        // Should not fail validation (but will fail because table doesn't exist, which is fine)
        if let Err(e) = result {
            assert!(!e.to_string().contains("Invalid table name format"));
        }

        // Invalid name (SQL injection attempt)
        let result = column_exists(&manager, "table; DROP TABLE users", "col").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid table name format"));

        // Empty name
        let result = column_exists(&manager, "", "col").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid table name format"));

        // Debug formatting check
        let result = column_exists(&manager, "invalid space", "col").await;
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("\"invalid space\"")); // Check for debug quotes
    }
}
