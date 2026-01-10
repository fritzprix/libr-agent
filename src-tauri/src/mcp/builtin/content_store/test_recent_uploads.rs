#[cfg(test)]
mod tests {
    use super::super::server::ContentStoreServer;
    use super::super::storage::ContentStoreStorage;
    use crate::session::SessionManager;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::Mutex;

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
        let session_manager =
            Arc::new(SessionManager::new_with_base_dir(temp_dir.path().to_path_buf()).unwrap());

        // Create server with pre-initialized storage
        let server = ContentStoreServer {
            session_id: session_id.clone(),
            session_manager,
            storage: Mutex::new(storage),
            search_engine: Arc::new(Mutex::new(
                super::super::search::ContentSearchEngine::new(
                    temp_dir.path().join("search_index"),
                )
                .unwrap(),
            )),
            recent_uploads: Arc::new(Mutex::new(std::collections::VecDeque::with_capacity(10))),
        };

        (server, temp_dir, session_id)
    }

    #[tokio::test]
    async fn test_recent_uploads_tracking() {
        let (server, _temp_dir, _session_id) = setup_server().await;

        // Add content using handler (this should track the upload)
        let args = serde_json::json!({
            "content": "Test content for recent uploads",
            "metadata": {
                "filename": "test.txt",
                "mimeType": "text/plain"
            }
        });

        let result = server.handle_save_knowledge(args).await;
        assert!(result.is_ok(), "Failed to save content");

        // Check recent uploads queue
        let recent = server.recent_uploads.lock().await;
        assert_eq!(recent.len(), 1, "Should have 1 recent upload");

        let upload = recent.front().unwrap();
        assert_eq!(upload.filename, "test.txt");
        assert_eq!(upload.mime_type, "text/plain");
        assert_eq!(upload.line_count, 1);
    }

    #[tokio::test]
    async fn test_service_context_includes_recent_uploads() {
        let (server, _temp_dir, _session_id) = setup_server().await;

        // Initially empty
        let context = server.get_service_context(None).await;
        assert!(
            context.context_prompt.contains("No files uploaded yet"),
            "Should show no files message"
        );

        // Add content
        let args = serde_json::json!({
            "content": "Test content line 1\nTest content line 2\nTest content line 3",
            "metadata": {
                "filename": "multiline.txt",
                "mimeType": "text/plain"
            }
        });

        server.handle_save_knowledge(args).await.unwrap();

        // Check service context now includes file
        let context = server.get_service_context(None).await;
        assert!(
            context.context_prompt.contains("Recent Uploads"),
            "Should show Recent Uploads section"
        );
        assert!(
            context.context_prompt.contains("multiline.txt"),
            "Should show filename"
        );
        assert!(
            context.context_prompt.contains("3 lines"),
            "Should show line count"
        );
        assert!(
            context.context_prompt.contains("text"),
            "Should show formatted mime type"
        );
    }

    #[tokio::test]
    async fn test_recent_uploads_fifo_limit() {
        let (server, _temp_dir, _session_id) = setup_server().await;

        // Add 12 files (should keep only last 10)
        for i in 1..=12 {
            let args = serde_json::json!({
                "content": format!("Content {}", i),
                "metadata": {
                    "filename": format!("file_{}.txt", i),
                    "mimeType": "text/plain"
                }
            });

            server.handle_save_knowledge(args).await.unwrap();
        }

        // Check queue size is limited to 10
        let recent = server.recent_uploads.lock().await;
        assert_eq!(recent.len(), 10, "Should keep only 10 most recent uploads");

        // First upload should be file_12.txt (most recent)
        let newest = recent.front().unwrap();
        assert_eq!(newest.filename, "file_12.txt");

        // Last upload should be file_3.txt (10th most recent)
        let oldest = recent.back().unwrap();
        assert_eq!(oldest.filename, "file_3.txt");
    }

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
