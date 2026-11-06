use crate::common;

use migration::MigratorTrait;
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sea_orm::{ConnectionTrait, DatabaseBackend, SqlxSqliteConnector, Statement};
use std::path::Path;
use std::str::FromStr;
use tauri_mcp_agent_lib::lifecycle::database::{
    ensure_schema_not_from_future, init_database, inspect_user_data_summary,
    maybe_restore_quarantined_database,
};
use tauri_mcp_agent_lib::lifecycle::database_error::DatabaseError;
use tauri_mcp_agent_lib::lifecycle::should_quarantine_on_init_failure;
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::utils::sqlite::format_sqlite_url;

async fn open_file_db(path: &Path) -> sea_orm::DatabaseConnection {
    common::register_sqlite_vec();
    let url = format_sqlite_url(&path.to_string_lossy());
    let options = SqliteConnectOptions::from_str(&url)
        .expect("Invalid database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("Failed to create test pool");

    SqlxSqliteConnector::from_sqlx_sqlite_pool(pool)
}

async fn seed_session(db: &sea_orm::DatabaseConnection, session_id: &str) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO sessions (id, status, created_at, updated_at) VALUES (?, ?, ?, ?)",
        [session_id.into(), "idle".into(), 1_i64.into(), 1_i64.into()],
    ))
    .await
    .expect("Failed to seed session");
}

#[tokio::test]
async fn restores_quarantined_database_when_active_db_is_empty() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let active_path = temp_dir.path().join("libragent_v2.db");
    let quarantined_path = temp_dir.path().join("libragent_v2.db.incompatible");

    let active_db = open_file_db(&active_path).await;
    let quarantined_db = open_file_db(&quarantined_path).await;

    Migrator::up(&active_db, None)
        .await
        .expect("Active DB migrations should succeed");
    Migrator::up(&quarantined_db, None)
        .await
        .expect("Quarantined DB migrations should succeed");

    seed_session(&quarantined_db, "session-from-quarantine").await;

    let active_url = format_sqlite_url(&active_path.to_string_lossy());
    maybe_restore_quarantined_database(&active_url)
        .await
        .expect("Quarantined DB restore should succeed");

    let restored_summary = inspect_user_data_summary(&active_path.to_string_lossy())
        .await
        .expect("Should inspect restored DB");
    assert_eq!(restored_summary.sessions, 1);
    assert!(
        quarantined_path.exists(),
        "Quarantined DB should remain as backup"
    );
}

#[tokio::test]
async fn skips_quarantined_restore_when_active_db_already_has_user_data() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let active_path = temp_dir.path().join("libragent_v2.db");
    let quarantined_path = temp_dir.path().join("libragent_v2.db.incompatible");

    let active_db = open_file_db(&active_path).await;
    let quarantined_db = open_file_db(&quarantined_path).await;

    Migrator::up(&active_db, None)
        .await
        .expect("Active DB migrations should succeed");
    Migrator::up(&quarantined_db, None)
        .await
        .expect("Quarantined DB migrations should succeed");

    seed_session(&active_db, "active-session").await;
    seed_session(&quarantined_db, "quarantined-session").await;

    let active_url = format_sqlite_url(&active_path.to_string_lossy());
    maybe_restore_quarantined_database(&active_url)
        .await
        .expect("Restore helper should not fail");

    let db = open_file_db(&active_path).await;
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT id FROM sessions LIMIT 1".to_string(),
        ))
        .await
        .expect("Should query sessions")
        .expect("Should keep one session row");

    let session_id: String = row.try_get("", "id").expect("Should read session id");
    assert_eq!(session_id, "active-session");
}

#[tokio::test]
async fn passes_future_schema_check_for_current_schema() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("libragent_v2.db");
    let db = open_file_db(&db_path).await;

    Migrator::up(&db, None)
        .await
        .expect("Migrations should succeed");

    ensure_schema_not_from_future(&db, &db_path.to_string_lossy())
        .await
        .expect("Current schema must pass the future-schema check");
}

#[tokio::test]
async fn refuses_database_with_future_schema() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("libragent_v2.db");
    let db = open_file_db(&db_path).await;

    Migrator::up(&db, None)
        .await
        .expect("Migrations should succeed");

    // Simulate a DB migrated by a newer build: a recorded migration this
    // build's Migrator does not know.
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO seaql_migrations (version, applied_at) VALUES (?, ?)",
        [
            "m20990101_000001_from_the_future".into(),
            1_000_000_000_i64.into(),
        ],
    ))
    .await
    .expect("Should insert future migration row");

    let error = ensure_schema_not_from_future(&db, &db_path.to_string_lossy())
        .await
        .expect_err("Future schema must be refused");

    match &error {
        DatabaseError::SchemaTooNew {
            unknown_migrations, ..
        } => {
            assert!(
                unknown_migrations.contains("m20990101_000001_from_the_future"),
                "Error must name the unknown migration, got: {unknown_migrations}"
            );
        }
        other => panic!("Expected SchemaTooNew, got: {other:?}"),
    }

    // The error message must name the DB path so the user knows which file to update for.
    assert!(
        error
            .to_string()
            .contains(&db_path.to_string_lossy().to_string()),
        "Error message must name the DB path, got: {error}"
    );

    // The message must also name this build's latest known migration so
    // support can compare the two schemas at a glance.
    assert!(
        error.to_string().contains("knows migrations only up to"),
        "Error message must name this build's latest known migration, got: {error}"
    );

    // Refusing must not touch the file: still a valid, queryable DB, and no
    // quarantine copy was created.
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) as c FROM seaql_migrations".to_string(),
        ))
        .await
        .expect("DB must remain queryable after refusal");
    let count: i64 = row
        .unwrap()
        .try_get("", "c")
        .expect("Should read migration count");
    assert!(count > 0);
    assert!(!Path::new(&format!("{}.incompatible", db_path.to_string_lossy())).exists());
}

#[test]
fn only_quarantines_for_structural_database_failures() {
    assert!(!should_quarantine_on_init_failure(
        &DatabaseError::SchemaTooNew {
            db_path: "/data/libragent_v2.db".to_string(),
            build_version: "0.9.0".to_string(),
            latest_known_migration: "m20260320_000021".to_string(),
            unknown_migrations: "m20990101_000001_from_the_future".to_string(),
        },
    ));

    // A failed pre-migration backup aborts startup — it must never rename
    // the user's database (issue #2618).
    assert!(!should_quarantine_on_init_failure(
        &DatabaseError::BackupFailed {
            path: "/data/libragent_v2.db".to_string(),
            error: "disk full".to_string(),
        }
    ));

    assert!(!should_quarantine_on_init_failure(
        &DatabaseError::ConnectionFailed("database is locked".to_string(),)
    ));

    assert!(should_quarantine_on_init_failure(
        &DatabaseError::MigrationModified {
            migration: "m20260320_000021".to_string(),
            expected_hash: "expected".to_string(),
            found_hash: "found".to_string(),
        }
    ));

    assert!(should_quarantine_on_init_failure(
        &DatabaseError::MigrationFailed {
            migration: "m20260320_000021".to_string(),
            error: "boom".to_string(),
            backup_path: None,
        }
    ));
}

/// Issue #2618: when the pre-migration backup fails, `init_database` must
/// abort with `BackupFailed` *before* migrating, must leave no backup file
/// behind, and a backup failure must never trigger quarantine (rename).
#[cfg(unix)]
#[tokio::test]
async fn aborts_startup_when_pre_migration_backup_fails() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("libragent_v2.db");
    let db_url = format_sqlite_url(&db_path.to_string_lossy());

    // Fully migrate the DB, then forget one applied migration so there is a
    // pending migration — this is the path that takes a backup first.
    let db = open_file_db(&db_path).await;
    Migrator::up(&db, None)
        .await
        .expect("Migrations should succeed");

    let oldest = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT version FROM seaql_migrations ORDER BY version LIMIT 1".to_string(),
        ))
        .await
        .expect("Should read oldest migration")
        .expect("Should have at least one migration");
    let removed_version: String = oldest.try_get("", "version").expect("Should read version");
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "DELETE FROM seaql_migrations WHERE version = ?",
        [removed_version.clone().into()],
    ))
    .await
    .expect("Should forget one applied migration");
    drop(db);

    // Make the backup directory unwritable so VACUUM INTO cannot create a file.
    let backup_dir = db_path
        .parent()
        .expect("DB has a parent dir")
        .join("backups");
    std::fs::create_dir_all(&backup_dir).expect("Should create backup dir");
    std::fs::set_permissions(&backup_dir, std::fs::Permissions::from_mode(0o555))
        .expect("Should make backup dir read-only");

    let probe = backup_dir.join("probe");
    if std::fs::File::create(&probe).is_ok() {
        // Root (or a permissive filesystem) ignores the read-only directory, so
        // the failure cannot be simulated here — skip rather than fake a pass.
        let _ = std::fs::remove_file(&probe);
        let _ = std::fs::set_permissions(&backup_dir, std::fs::Permissions::from_mode(0o755));
        eprintln!("Skipping: environment ignores read-only directory permissions");
        return;
    }

    let result = init_database(&db_url).await;

    // Restore permissions before asserting so the temp dir can always be cleaned.
    let _ = std::fs::set_permissions(&backup_dir, std::fs::Permissions::from_mode(0o755));

    let error = result
        .err()
        .expect("init_database must fail when the pre-migration backup fails");
    assert!(
        matches!(error, DatabaseError::BackupFailed { .. }),
        "Expected BackupFailed, got: {error:?}"
    );
    // A backup failure must never rename the user's database aside.
    assert!(!should_quarantine_on_init_failure(&error));

    // No backup file — complete or partial — may be left behind.
    let leftovers: Vec<std::path::PathBuf> = std::fs::read_dir(&backup_dir)
        .expect("backup dir should be readable")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();
    assert!(
        leftovers.is_empty(),
        "Failed backup must leave no file behind, found: {leftovers:?}"
    );

    // The migration must NOT have run: the forgotten row must still be missing.
    let db = open_file_db(&db_path).await;
    let row = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) as c FROM seaql_migrations WHERE version = ?",
            [removed_version.clone().into()],
        ))
        .await
        .expect("Should query migrations after abort");
    let count: i64 = row
        .expect("query row")
        .try_get("", "c")
        .expect("Should read count");
    assert_eq!(
        count, 0,
        "Migration {removed_version} must not have been re-applied after the backup abort"
    );

    // The DB must still be intact and queryable (never quarantined/renamed).
    assert!(db_path.exists());
    assert!(!Path::new(&format!("{}.incompatible", db_path.to_string_lossy())).exists());
}
