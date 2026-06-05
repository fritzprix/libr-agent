use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
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
    Database::connect(opt)
        .await
        .expect("Failed to create in-memory database")
}

#[allow(dead_code)]
pub async fn setup_test_db_with_migrations() -> DatabaseConnection {
    let db = setup_test_db().await;
    Migrator::up(&db, None)
        .await
        .expect("Migrations should run");
    db
}
