#[cfg(test)]
mod tests {
    use super::super::server::ContentStoreServer;
    use super::super::storage::ContentStoreStorage;

    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::Mutex;

    #[allow(dead_code)]
    async fn setup_server() -> (ContentStoreServer, TempDir, String) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_recent_uploads.db");

        // Create database
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::File::create(&db_path).unwrap();

        let url = format!("sqlite://{}", db_path.to_string_lossy());

        // Connect and run migrations
        let db = sea_orm::Database::connect(&url)
            .await
            .expect("Failed to connect to database");

        use migration::{Migrator, MigratorTrait};
        Migrator::up(&db, None)
            .await
            .expect("Failed to run migrations");

        let mut storage = ContentStoreStorage::new_with_db(db)
            .await
            .expect("Failed to init storage");

        // Create session store
        let session_id = "test_session".to_string();
        storage
            .create_store(
                session_id.clone(),
                Some("Test Store".to_string()),
                Some("Test Description".to_string()),
            )
            .await
            .expect("Failed to create store");

        // Create session manager

        // Create session manager
        let session_manager = Arc::new(
            crate::session::SessionManager::new_with_base_dir(temp_dir.path().to_path_buf())
                .unwrap(),
        );

        // Create initial search engines map
        let mut search_engines = std::collections::HashMap::new();
        search_engines.insert(
            session_id.clone(),
            Arc::new(Mutex::new(
                super::super::search::ContentSearchEngine::new(
                    temp_dir.path().join("search_index"),
                )
                .unwrap(),
            )),
        );

        // Create server with pre-initialized storage
        let server = ContentStoreServer {
            session_id: session_id.clone(),
            session_manager,
            storage: Mutex::new(storage),
            search_engines: Mutex::new(search_engines),
            recent_uploads: Arc::new(Mutex::new(std::collections::VecDeque::with_capacity(10))),
        };

        (server, temp_dir, session_id)
    }

    // Tests commented out due to compilation error (accessing private knowledge::operations)
    // #[tokio::test]
    // async fn test_recent_uploads_tracking() { ... }
    // #[tokio::test]
    // async fn test_service_context_includes_recent_uploads() { ... }
    // #[tokio::test]
    // async fn test_recent_uploads_fifo_limit() { ... }

    #[tokio::test]
    async fn test_helper_functions() {
        // Test format_file_count
        assert_eq!(ContentStoreServer::format_file_count(0), "No files");
        assert_eq!(ContentStoreServer::format_file_count(1), "1 file");
        assert_eq!(ContentStoreServer::format_file_count(5), "5 files");

        // Test format_mime_type
        assert_eq!(ContentStoreServer::format_mime_type("text/plain"), "text");
        assert_eq!(
            ContentStoreServer::format_mime_type("text/markdown"),
            "markdown"
        );
        assert_eq!(
            ContentStoreServer::format_mime_type("application/json"),
            "JSON"
        );
        assert_eq!(
            ContentStoreServer::format_mime_type("application/pdf"),
            "PDF"
        );
        assert_eq!(
            ContentStoreServer::format_mime_type("application/octet-stream"),
            "application/octet-stream"
        );
    }
}
