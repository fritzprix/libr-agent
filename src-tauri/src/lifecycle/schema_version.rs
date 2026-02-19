use log::{info, warn};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, Statement};
use serde::{Deserialize, Serialize};

/// Snapshot of the `schema_version` table row created by migration #10.
///
/// Columns: version (PK, string), migration_count (integer), applied_at (bigint), checksum (string nullable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaVersionRecord {
    /// Semver string from Cargo.toml at migration time (e.g. "0.5.3")
    pub version: String,
    /// Number of migrations applied at this version
    pub migration_count: i32,
    /// Unix timestamp (milliseconds) when this version was recorded
    pub applied_at: i64,
    /// MD5 of all migration version names joined by comma (integrity signal)
    pub checksum: Option<String>,
}

/// Returns the single current schema version record, or `None` if the table
/// has not been created yet (i.e. migration #10 has not run).
pub async fn get_current_schema_version(
    db: &DatabaseConnection,
) -> Result<Option<SchemaVersionRecord>, DbErr> {
    let backend = db.get_database_backend();

    // Guard: table is only created by migration #10
    let has_table = db
        .query_one(Statement::from_string(
            backend,
            "SELECT name FROM sqlite_master WHERE type='table' AND name='schema_version'"
                .to_string(),
        ))
        .await?;

    if has_table.is_none() {
        info!("📊 schema_version table not yet created (migration #10 pending)");
        return Ok(None);
    }

    let row = db
        .query_one(Statement::from_string(
            backend,
            // The table stores one canonical row (DELETE + INSERT pattern in migration_verifier).
            "SELECT version, migration_count, applied_at, checksum FROM schema_version LIMIT 1"
                .to_string(),
        ))
        .await?;

    match row {
        Some(r) => Ok(Some(SchemaVersionRecord {
            version: r.try_get("", "version")?,
            migration_count: r.try_get("", "migration_count")?,
            applied_at: r.try_get("", "applied_at")?,
            checksum: r.try_get("", "checksum").ok().flatten(),
        })),
        None => Ok(None),
    }
}

/// Verify that the recorded migration count is at least `expected_min`.
///
/// Returns `true` when the check passes, `false` when the schema is behind.
pub async fn verify_schema_migration_count(
    db: &DatabaseConnection,
    expected_min: i32,
) -> Result<bool, DbErr> {
    match get_current_schema_version(db).await? {
        Some(rec) if rec.migration_count >= expected_min => {
            info!(
                "✅ Schema migration count OK: {} >= {}",
                rec.migration_count, expected_min
            );
            Ok(true)
        }
        Some(rec) => {
            warn!(
                "⚠️ Schema migration count too low: {} < {} (version {})",
                rec.migration_count, expected_min, rec.version
            );
            Ok(false)
        }
        None => {
            warn!("⚠️ schema_version table absent — migration #10 has not run yet");
            Ok(false)
        }
    }
}

/// Log a human-readable schema version summary (useful at startup).
pub async fn display_schema_info(db: &DatabaseConnection) -> Result<(), DbErr> {
    info!("📊 ===== Schema Version Info =====");
    match get_current_schema_version(db).await? {
        Some(rec) => {
            info!("📌 App version at last migration : {}", rec.version);
            info!("📌 Migrations applied            : {}", rec.migration_count);
            let ts = chrono::DateTime::from_timestamp_millis(rec.applied_at)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "unknown".to_string());
            info!("📌 Recorded at                   : {}", ts);
            if let Some(cs) = &rec.checksum {
                info!("📌 Checksum                      : {}", cs);
            }
        }
        None => {
            info!("📌 No schema version recorded yet");
        }
    }
    info!("📊 ================================");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DatabaseConnection};

    async fn open_in_memory() -> DatabaseConnection {
        Database::connect("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn returns_none_when_table_absent() {
        let db = open_in_memory().await;
        let result = get_current_schema_version(&db).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn verify_count_returns_false_when_absent() {
        let db = open_in_memory().await;
        let ok = verify_schema_migration_count(&db, 0).await.unwrap();
        assert!(!ok);
    }
}
