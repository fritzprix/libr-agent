use migration::Migrator;
use sea_orm::{DatabaseConnection, SqlxSqliteConnector};
use sqlx::SqlitePool;

/// Initialize database tables and indexes using SeaORM Migrations
pub async fn init_tables(pool: &SqlitePool, session_id: &str) -> Result<(), String> {
    use sea_orm_migration::MigratorTrait;

    // Convert SqlitePool to SeaORM DatabaseConnection
    let db: DatabaseConnection = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool.clone());

    // Run migrations
    let result: Result<(), sea_orm_migration::DbErr> = Migrator::up(&db, None).await;
    result.map_err(|e| format!("Failed to run migrations: {}", e))?;

    log::debug!(
        "Planning server tables synced and initialized for session: {}",
        session_id
    );

    Ok(())
}
