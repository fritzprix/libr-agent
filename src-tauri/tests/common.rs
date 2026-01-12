use sea_orm::{Database, DatabaseConnection, DbErr};

/// Setup an in-memory SQLite database for testing
pub async fn setup_test_db() -> DatabaseConnection {
    Database::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database")
}
