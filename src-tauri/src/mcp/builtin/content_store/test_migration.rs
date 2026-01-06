#[cfg(test)]
mod tests {
    use crate::mcp::builtin::content_store::storage::ContentStoreStorage;
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
    use std::fs;

    async fn setup_old_db(db_path: &str) -> DatabaseConnection {
        // Create DB file
        let url = format!("sqlite://{}", db_path);
        let db = Database::connect(&url)
            .await
            .expect("Failed to connect to DB");

        // Create table WITHOUT src_url
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE IF NOT EXISTS contents (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                uri TEXT NOT NULL,
                mime_type TEXT NOT NULL,
                title TEXT,
                tags TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                uploaded_at INTEGER NOT NULL
            )"
            .to_owned(),
        ))
        .await
        .expect("Failed to create old table");

        db
    }

    #[tokio::test]
    async fn test_migration_adds_src_url() {
        let db_path = "test_migration.db";
        // Clean up previous run
        let _ = fs::remove_file(db_path);
        // Create empty file for SQLite
        fs::File::create(db_path).expect("Failed to create db file");

        // 1. Setup old DB schema
        let db = setup_old_db(db_path).await;
        db.close().await.expect("Failed to close DB");

        // 2. Initialize ContentStoreStorage (should trigger migration)
        let url = format!("sqlite://{}", db_path);
        let storage_result = ContentStoreStorage::new_sqlite(url.clone()).await;
        assert!(
            storage_result.is_ok(),
            "Failed to init storage: {:?}",
            storage_result.err()
        );

        // 3. Verify column exists by querying it
        let db = Database::connect(&url)
            .await
            .expect("Failed to connect to DB");

        // Try to insert a row with src_url manually to verify column exists
        let result = db
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE contents SET src_url = 'http://test.com' WHERE id = 'nonexistent'"
                    .to_owned(),
            ))
            .await;

        // If column doesn't exist, this would fail
        assert!(
            result.is_ok(),
            "Migration failed: src_url column likely missing. Error: {:?}",
            result.err()
        );

        // Clean up
        db.close().await.expect("Failed to close DB");
        let _ = fs::remove_file(db_path);
    }
}
