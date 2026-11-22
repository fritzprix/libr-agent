use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(windows))]
use std::sync::Once;
use tauri_mcp_agent_lib::migration::Migrator;

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn register_sqlite_vec() {
    #[cfg(not(windows))]
    {
        static REGISTER_SQLITE_VEC: Once = Once::new();

        REGISTER_SQLITE_VEC.call_once(|| unsafe {
            libsqlite3_sys::sqlite3_auto_extension(Some(std::mem::transmute::<
                *const (),
                unsafe extern "C" fn(
                    *mut libsqlite3_sys::sqlite3,
                    *mut *mut i8,
                    *const libsqlite3_sys::sqlite3_api_routines,
                ) -> i32,
            >(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        });
    }
}

/// A fresh fixture directory under `target/`, deliberately outside the OS temp
/// directory (#2946 makes the OS temp directory an accepted file-tool root, so a
/// `tempdir()` fixture can no longer stand in for "a directory nobody allowed";
/// on macOS `tempdir()` is also the non-canonical `/var/folders/…` form of that
/// root, which matches nothing). Unique per process; stale remnants are cleared.
#[allow(dead_code)]
pub fn outside_os_temp_dir(tag: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("outside-os-temp-fixtures")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("fixture dir");
    dir
}

/// Setup an isolated shared-memory SQLite database for testing.
pub async fn setup_test_db() -> DatabaseConnection {
    tauri_mcp_agent_lib::reset_state();
    register_sqlite_vec();
    let db_id = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
    let database_url = format!("sqlite::file:test_db_{db_id}?mode=memory&cache=shared");
    let mut opt = sea_orm::ConnectOptions::new(database_url);
    opt.max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(opt)
        .await
        .expect("Failed to create in-memory database");

    // Match production: enforce FK / ON DELETE CASCADE on every connection.
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "PRAGMA foreign_keys = ON".to_string(),
    ))
    .await
    .expect("Failed to enable SQLite foreign_keys");

    db
}

#[allow(dead_code)]
pub async fn setup_test_db_with_migrations() -> DatabaseConnection {
    let db = setup_test_db().await;
    Migrator::up(&db, None)
        .await
        .expect("Migrations should run");

    // Seed default assistants for integration tests
    register_assistant_repository(&db);
    tauri_mcp_agent_lib::services::assistant_init::ensure_default_assistants(None)
        .await
        .expect("Failed to seed default assistants for integration tests");

    db
}

/// Register the assistant repository against global state for resolve_agent_config().
#[allow(dead_code)]
pub fn register_assistant_repository(db: &DatabaseConnection) {
    use tauri_mcp_agent_lib::repositories::SqliteAssistantRepository;
    use tauri_mcp_agent_lib::set_assistant_repository;

    set_assistant_repository(SqliteAssistantRepository::new(db.clone()));
}

/// Insert an assistant row used by session SSOT resolution in integration tests.
#[allow(dead_code)]
pub async fn seed_test_assistant(
    db: &DatabaseConnection,
    assistant_id: &str,
    name: &str,
    config: serde_json::Value,
) {
    use tauri_mcp_agent_lib::repositories::{AssistantRepository, SqliteAssistantRepository};

    let repo = SqliteAssistantRepository::new(db.clone());
    if repo
        .get_assistant(assistant_id)
        .await
        .expect("assistant lookup should succeed")
        .is_some()
    {
        return;
    }

    repo.create_assistant(
        assistant_id.to_string(),
        name.to_string(),
        config.to_string(),
    )
    .await
    .expect("assistant should seed");
}
