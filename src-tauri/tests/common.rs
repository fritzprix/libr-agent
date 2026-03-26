use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use std::sync::Once;
use tauri_mcp_agent_lib::migration::Migrator;

pub fn register_sqlite_vec() {
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
            sqlite_vec::sqlite3_vec_init as *const ()
        )));
    });
}

/// Setup an in-memory SQLite database for testing
#[allow(dead_code)]
pub async fn setup_test_db() -> DatabaseConnection {
    register_sqlite_vec();
    Database::connect("sqlite::memory:")
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
