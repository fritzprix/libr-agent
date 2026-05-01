pub mod common;

use migration::MigratorTrait;
use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sea_orm::{ConnectionTrait, DatabaseBackend, SqlxSqliteConnector, Statement};
use std::path::Path;
use std::str::FromStr;
use tauri_mcp_agent_lib::lifecycle::database::{
    inspect_user_data_summary, maybe_restore_quarantined_database,
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

#[test]
fn only_quarantines_for_structural_database_failures() {
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
