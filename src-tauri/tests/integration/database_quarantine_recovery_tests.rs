use crate::common;

use migration::MigratorTrait;
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sea_orm::{ConnectionTrait, DatabaseBackend, SqlxSqliteConnector, Statement};
use std::path::Path;
use std::str::FromStr;
use tauri_mcp_agent_lib::lifecycle::database::{
    ensure_schema_not_from_future, inspect_user_data_summary, maybe_restore_quarantined_database,
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
            unknown_migrations: "m20990101_000001_from_the_future".to_string(),
        },
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
