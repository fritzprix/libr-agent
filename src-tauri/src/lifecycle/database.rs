use crate::db_schema_validator::validate_schema;
use crate::lifecycle::schema_version;
use crate::migration::{Migrator, MigratorTrait};
use log::{error, info, warn};
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sea_orm::DatabaseConnection;
use sea_orm::SqlxSqliteConnector;
use sha2::{Digest, Sha256};
use std::str::FromStr;
use std::time::{Duration, Instant};

use super::database_backup::BackupManager;
use super::database_error::{DatabaseError, DatabaseResult};
use super::migration_verifier::MigrationVerifier;
#[cfg(test)]
use super::retry_utils::retry_with_backoff;

/// Helper function to safely remove database file by renaming it
#[cfg(test)]
fn remove_db_file(file_path: &str) -> DatabaseResult<()> {
    let backup = format!("{}.old", file_path);

    // Try to remove existing backup first
    if std::path::Path::new(&backup).exists() {
        if let Err(e) = std::fs::remove_file(&backup) {
            warn!("⚠️ Failed to remove existing backup: {}", e);
        }
    }

    // Try to rename with retry and exponential backoff
    retry_with_backoff(
        || std::fs::rename(file_path, &backup),
        5,   // 5 attempts
        100, // Start with 100ms
    )
    .map_err(|e| {
        // Check for Windows file locking (error code 32)
        if e.raw_os_error() == Some(32) || e.kind() == std::io::ErrorKind::PermissionDenied {
            DatabaseError::FileLocked {
                path: file_path.to_string(),
                attempts: 5,
            }
        } else {
            DatabaseError::IoError(e)
        }
    })?;

    info!("✅ Database file moved to: {}", backup);
    Ok(())
}

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

pub async fn init_database(db_url: &str) -> DatabaseResult<DatabaseConnection> {
    // Extract file path from URL (strip sqlite:// prefix and query params)
    let db_file_path = extract_db_file_path(db_url)
        .ok_or_else(|| DatabaseError::ConnectionFailed("Invalid database URL format".into()))?;

    // Create backup manager
    let backup_manager = BackupManager::new(db_file_path);

    // Build SqliteConnectOptions with WAL mode and busy timeout.
    // SeaORM's SQLite driver does NOT support journal_mode/busy_timeout as URL
    // query parameters — they must be set via SqliteConnectOptions.
    let sqlite_opts = SqliteConnectOptions::from_str(&format!("sqlite://{db_file_path}"))
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

    // Create backup before migration
    info!("📦 Creating backup before migration...");
    let backup_path = backup_manager.create_backup().ok();

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_remove_db_file_success() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_db_remove.db");
        let backup_file = format!("{}.old", test_file.display());

        let mut file = fs::File::create(&test_file).unwrap();
        file.write_all(b"test data").unwrap();
        drop(file);

        assert!(test_file.exists());

        let _ = remove_db_file(test_file.to_str().unwrap());

        assert!(!test_file.exists(), "Original file should not exist");
        assert!(
            std::path::Path::new(&backup_file).exists(),
            "Backup file should exist"
        );

        let _ = fs::remove_file(&backup_file);
    }

    #[test]
    fn test_remove_db_file_with_existing_backup() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_db_existing_backup.db");
        let backup_file = format!("{}.old", test_file.display());

        fs::File::create(&test_file).unwrap();
        fs::File::create(&backup_file).unwrap();

        let _ = remove_db_file(test_file.to_str().unwrap());

        assert!(!test_file.exists());
        assert!(std::path::Path::new(&backup_file).exists());

        let _ = fs::remove_file(&backup_file);
    }
}
