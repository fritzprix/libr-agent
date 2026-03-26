use crate::db_schema_validator::validate_schema;
use crate::lifecycle::schema_version;
use crate::migration::{Migrator, MigratorTrait};
use log::{error, info, warn};
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, SqlxSqliteConnector, Statement,
};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, Instant};

use super::database_backup::BackupManager;
use super::database_error::{DatabaseError, DatabaseResult};
use super::migration_verifier::MigrationVerifier;

/// Extract the filesystem path from a `sqlite://` URL, stripping any query parameters.
///
/// Returns `None` when the URL does not start with `sqlite://`.
/// This is a module-level helper so callers (e.g. `mod.rs` quarantine logic)
/// can reuse the same extraction logic without duplicating the string manipulation.
pub fn extract_db_file_path(db_url: &str) -> Option<&str> {
    db_url
        .strip_prefix("sqlite://")
        .and_then(|p| p.split('?').next())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UserDataSummary {
    pub sessions: i64,
    pub messages: i64,
    pub planning_goals: i64,
    pub planning_todos: i64,
    pub settings: i64,
    pub mcp_servers: i64,
    pub assistants: i64,
}

impl UserDataSummary {
    pub fn meaningful_score(&self) -> i64 {
        self.sessions
            + self.messages
            + self.planning_goals
            + self.planning_todos
            + self.settings
            + self.mcp_servers
    }

    pub fn has_meaningful_user_data(&self) -> bool {
        self.meaningful_score() > 0
    }
}

fn validate_sqlite_identifier(identifier: &str) -> DatabaseResult<()> {
    if identifier.is_empty()
        || !identifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(DatabaseError::ConnectionFailed(format!(
            "Invalid SQLite identifier format: {identifier:?}"
        )));
    }

    Ok(())
}

async fn connect_existing_database(db_file_path: &str) -> DatabaseResult<DatabaseConnection> {
    let db_url_formatted = crate::utils::sqlite::format_sqlite_url(db_file_path);
    let sqlite_opts = SqliteConnectOptions::from_str(&db_url_formatted)
        .map_err(|e| DatabaseError::ConnectionFailed(format!("Invalid SQLite path: {e}")))?
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5))
        .create_if_missing(false);

    let sqlx_pool = sea_orm::sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlite_opts)
        .await
        .map_err(|e| DatabaseError::ConnectionFailed(e.to_string()))?;

    Ok(SqlxSqliteConnector::from_sqlx_sqlite_pool(sqlx_pool))
}

async fn table_exists(db: &DatabaseConnection, table_name: &str) -> DatabaseResult<bool> {
    validate_sqlite_identifier(table_name)?;

    let result = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type='table' AND name=?",
            [table_name.into()],
        ))
        .await?;

    Ok(result.is_some())
}

async fn count_rows_if_table_exists(
    db: &DatabaseConnection,
    table_name: &str,
) -> DatabaseResult<i64> {
    validate_sqlite_identifier(table_name)?;

    if !table_exists(db, table_name).await? {
        return Ok(0);
    }

    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("SELECT COUNT(*) as row_count FROM {table_name}"),
        ))
        .await?;

    match row {
        Some(row) => row
            .try_get("", "row_count")
            .map_err(|e| DatabaseError::ConnectionFailed(format!("Failed to read row_count: {e}"))),
        None => Ok(0),
    }
}

async fn integrity_check_ok(db_file_path: &str) -> DatabaseResult<bool> {
    let db = connect_existing_database(db_file_path).await?;
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA integrity_check".to_string(),
        ))
        .await?;

    match row {
        Some(row) => {
            let result: String = row.try_get("", "integrity_check").map_err(|e| {
                DatabaseError::ConnectionFailed(format!(
                    "Failed to read integrity_check result: {e}"
                ))
            })?;
            Ok(result == "ok")
        }
        None => Ok(false),
    }
}

fn remove_sqlite_sidecars(db_file_path: &str) {
    for suffix in ["-wal", "-shm"] {
        let sidecar_path = format!("{db_file_path}{suffix}");
        if Path::new(&sidecar_path).exists() {
            if let Err(err) = std::fs::remove_file(&sidecar_path) {
                warn!(
                    "⚠️ Failed to remove SQLite sidecar {}: {}",
                    sidecar_path, err
                );
            }
        }
    }
}

pub async fn inspect_user_data_summary(db_file_path: &str) -> DatabaseResult<UserDataSummary> {
    if !Path::new(db_file_path).exists() {
        return Ok(UserDataSummary::default());
    }

    let db = connect_existing_database(db_file_path).await?;

    Ok(UserDataSummary {
        sessions: count_rows_if_table_exists(&db, "sessions").await?,
        messages: count_rows_if_table_exists(&db, "messages").await?,
        planning_goals: count_rows_if_table_exists(&db, "planning_goals").await?,
        planning_todos: count_rows_if_table_exists(&db, "planning_todos").await?,
        settings: count_rows_if_table_exists(&db, "settings").await?,
        mcp_servers: count_rows_if_table_exists(&db, "mcp_servers").await?,
        assistants: count_rows_if_table_exists(&db, "assistants").await?,
    })
}

pub async fn maybe_restore_quarantined_database(db_url: &str) -> DatabaseResult<()> {
    let db_file_path = match extract_db_file_path(db_url) {
        Some(path) => path,
        None => return Ok(()),
    };

    let quarantine_path = format!("{db_file_path}.incompatible");
    if !Path::new(db_file_path).exists() || !Path::new(&quarantine_path).exists() {
        return Ok(());
    }

    let current_summary = inspect_user_data_summary(db_file_path).await?;
    let quarantined_summary = inspect_user_data_summary(&quarantine_path).await?;

    if current_summary.has_meaningful_user_data() {
        info!(
            "ℹ️ Active DB already contains user data, skipping quarantined DB restore: current={:?} quarantined={:?}",
            current_summary, quarantined_summary
        );
        return Ok(());
    }

    if !quarantined_summary.has_meaningful_user_data() {
        info!(
            "ℹ️ Quarantined DB has no recoverable user data, skipping restore: {:?}",
            quarantined_summary
        );
        return Ok(());
    }

    if !integrity_check_ok(&quarantine_path).await? {
        warn!(
            "⚠️ Quarantined DB failed integrity_check, refusing automatic restore: {}",
            quarantine_path
        );
        return Ok(());
    }

    warn!(
        "⚠️ Restoring quarantined DB because active DB appears empty: current={:?} quarantined={:?}",
        current_summary, quarantined_summary
    );

    remove_sqlite_sidecars(db_file_path);
    std::fs::copy(&quarantine_path, db_file_path).map_err(DatabaseError::IoError)?;

    info!(
        "✅ Restored quarantined DB back to active path: {} <- {}",
        db_file_path, quarantine_path
    );

    Ok(())
}

pub async fn init_database(db_url: &str) -> DatabaseResult<DatabaseConnection> {
    // Register sqlite-vec extension before any database connections are opened
    unsafe {
        libsqlite3_sys::sqlite3_auto_extension(Some(std::mem::transmute::<
            *const (),
            unsafe extern "C" fn(
                *mut libsqlite3_sys::sqlite3,
                *mut *mut i8,
                *const libsqlite3_sys::sqlite3_api_routines,
            ) -> i32,
        >(
            sqlite_vec::sqlite3_vec_init as *const ()
        )));
    }

    // Extract file path from URL (strip sqlite:// prefix and query params)
    let db_file_path = extract_db_file_path(db_url)
        .ok_or_else(|| DatabaseError::ConnectionFailed("Invalid database URL format".into()))?;

    // Create backup manager
    let backup_manager = BackupManager::new(db_file_path);

    // Build SqliteConnectOptions with WAL mode and busy timeout.
    // SeaORM's SQLite driver does NOT support journal_mode/busy_timeout as URL
    // query parameters — they must be set via SqliteConnectOptions.
    let db_url_formatted = crate::utils::sqlite::format_sqlite_url(db_file_path);
    let sqlite_opts = SqliteConnectOptions::from_str(&db_url_formatted)
        .map_err(|e| DatabaseError::ConnectionFailed(format!("Invalid SQLite path: {e}")))?
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5))
        .create_if_missing(true);

    // Ensure parent directory exists before connecting
    if let Some(parent) = std::path::Path::new(db_file_path).parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            error!("Failed to create parent directory for DB: {err}");
        }
    }

    // Connect via SqlxSqliteConnector which accepts SqliteConnectOptions directly.
    // This is the correct way to use non-URL options (WAL, busy_timeout) with sea-orm.
    let sqlx_pool = sea_orm::sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1) // SQLite: single writer
        .connect_with(sqlite_opts)
        .await
        .map_err(|e| DatabaseError::ConnectionFailed(e.to_string()))?;

    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(sqlx_pool);

    info!("✅ Database connected (WAL mode): {db_file_path}");

    // Create backup before migration using VACUUM INTO (WAL-safe)
    info!("📦 Creating backup before migration...");
    let backup_path = backup_manager.create_backup(&db).await.ok();

    if let Some(ref path) = backup_path {
        info!("✅ Backup created: {}", path.display());
    }

    // Verify migration file integrity before running.
    // NOTE: Migration .rs source files are only available in development builds.
    // In release builds the source directory won't exist, so verification is skipped
    // gracefully. The MigrationVerifier already handles a missing directory with Ok(()).
    info!("🔍 Verifying migration file integrity...");
    let migration_dir = std::env::current_dir()
        .ok()
        .and_then(|mut p| {
            p.push("migration");
            p.push("src");
            if p.exists() {
                Some(p.to_string_lossy().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "./migration/src".to_string());

    let verifier = MigrationVerifier::new(db.clone(), migration_dir);

    // Verify existing migrations (skip on first run or when source dir is absent)
    match verifier.verify_all_migrations().await {
        Ok(()) => info!("✅ Migration integrity verified"),
        Err(verification_error) => {
            // In release builds the migration source directory is not bundled,
            // so a "directory not found" situation is expected and harmless.
            // Only treat as fatal if the verifier actually found a checksum mismatch.
            if verification_error.contains("was modified") {
                error!(
                    "❌ Migration integrity check failed: {}",
                    verification_error
                );
                return Err(DatabaseError::MigrationModified {
                    migration: "multiple".into(),
                    expected_hash: "see_log".into(),
                    found_hash: verification_error,
                });
            } else {
                warn!(
                    "⚠️ Migration verification skipped (source files unavailable): {}",
                    verification_error
                );
            }
        }
    }

    // Run migrations with timing
    info!("🚀 Running database migrations...");
    let start = Instant::now();
    let migration_result = Migrator::up(&db, None).await;
    let execution_time_ms = start.elapsed().as_millis() as i64;

    // Handle migration result
    let db = match migration_result {
        Ok(_) => {
            info!("✅ Database migrations applied ({}ms)", execution_time_ms);

            // Update schema version
            let version = env!("CARGO_PKG_VERSION");
            let migration_count = Migrator::migrations().len() as i32;

            // Compute overall checksum using SHA-256 (consistent with migration_verifier)
            let all_versions: Vec<String> = Migrator::migrations()
                .iter()
                .map(|m| m.name().to_string())
                .collect();

            let overall_checksum = {
                let mut hasher = Sha256::new();
                hasher.update(all_versions.join(",").as_bytes());
                format!("{:x}", hasher.finalize())
            };

            if let Err(e) = verifier
                .update_schema_version(version, migration_count, &overall_checksum)
                .await
            {
                warn!("⚠️ Failed to update schema version: {}", e);
            } else {
                info!(
                    "✅ Schema version updated: v{} ({} migrations)",
                    version, migration_count
                );
            }

            db
        }
        Err(e) => {
            error!("❌ Database migration failed: {}", e);

            // Return error with backup info instead of panic
            return Err(DatabaseError::MigrationFailed {
                migration: "unknown".into(),
                error: e.to_string(),
                backup_path: backup_path.map(|p| p.display().to_string()),
            });
        }
    };

    // Validate schema after migrations (warnings only, don't fail)
    if let Err(validation_err) = validate_schema(&db).await {
        warn!("⚠️ Schema validation failed: {}", validation_err);
        warn!("⚠️ Some features may not work correctly");
        // Don't fail — just warn and continue
    } else {
        info!("✅ Database schema validated");
    }

    // Log schema version info and check for migration count mismatch.
    let expected_count = Migrator::migrations().len() as i32;
    match schema_version::get_current_schema_version(&db).await {
        Ok(Some(rec)) => {
            if rec.migration_count != expected_count {
                warn!(
                    "⚠️ Migration count mismatch: schema_version records {} but Migrator has {} \
                    — schema_version will be refreshed on next successful startup",
                    rec.migration_count, expected_count
                );
            } else {
                info!(
                    "📊 Schema v{} — {} migrations (checksum: {})",
                    rec.version,
                    rec.migration_count,
                    rec.checksum.as_deref().unwrap_or("n/a")
                );
            }
        }
        Ok(None) => {
            info!("📊 schema_version not yet recorded (migration #10 pending or fresh install)");
        }
        Err(e) => {
            warn!("⚠️ Could not read schema_version: {}", e);
        }
    }

    Ok(db)
}
