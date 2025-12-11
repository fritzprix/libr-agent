#[cfg(test)]
mod tests {
    use crate::mcp::builtin::content_store::storage::ContentStoreStorage;
    use tempfile::TempDir;

    async fn setup_storage() -> (ContentStoreStorage, TempDir, String) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_functional.db");
        let url = format!("sqlite://{}", db_path.to_string_lossy());

        let mut storage = ContentStoreStorage::new_sqlite(url)
            .await
            .expect("Failed to init storage");

        // Create a test session store
        let session_id = "test_session".to_string();
        storage
            .create_store(
                session_id.clone(),
                Some("Test Store".to_string()),
                Some("Test Description".to_string()),
            )
            .await
            .expect("Failed to create store");

        (storage, temp_dir, session_id)
    }

    #[tokio::test]
    async fn test_full_lifecycle() {
        let (mut storage, _temp_dir, session_id) = setup_storage().await;

        // 1. Add Content
        let content = "This is a test content for functional testing.";
        let filename = "test.txt";
        let mime_type = "text/plain";
        let size = content.len();
        let chunks = vec![content.to_string()];
        let src_url = Some("http://example.com/test".to_string());

        let content_item = storage
            .add_content(
                &session_id,
                filename,
                mime_type,
                size,
                content,
                chunks,
                src_url.clone(),
            )
            .await
            .expect("Failed to add content");

        let content_id = content_item.id.clone();
        assert!(!content_id.is_empty(), "ID should not be empty");
        assert_eq!(content_item.src_url, src_url);

        // 2. Read Content
        let read_result = storage
            .read_content(&content_id, 1, None)
            .await
            .expect("Failed to read content");
        assert!(read_result.contains("functional testing"));

        // 3. List Content
        let (list_result, total) = storage
            .list_content(&session_id, 0, 10)
            .await
            .expect("Failed to list content");
        assert_eq!(list_result.len(), 1);
        assert_eq!(total, 1);
        assert_eq!(list_result[0].id, content_id);
        assert_eq!(list_result[0].src_url, src_url);

        // 4. Delete Content
        storage
            .delete_content(&content_id)
            .await
            .expect("Failed to delete content");

        // 5. Verify deletion
        let (list_after_delete, total_after) = storage
            .list_content(&session_id, 0, 10)
            .await
            .expect("Failed to list after delete");
        assert_eq!(list_after_delete.len(), 0);
        assert_eq!(total_after, 0);
    }

    #[tokio::test]
    async fn test_src_url_persistence() {
        let (mut storage, _temp_dir, session_id) = setup_storage().await;

        let src_url = Some("https://github.com/example/repo".to_string());

        let content_item = storage
            .add_content(
                &session_id,
                "github_file.md",
                "text/markdown",
                100,
                "# GitHub Content",
                vec!["# GitHub Content".to_string()],
                src_url.clone(),
            )
            .await
            .expect("Failed to add content");

        // Verify src_url is stored
        assert_eq!(content_item.src_url, src_url);

        // List and verify src_url persists
        let (list_result, _) = storage
            .list_content(&session_id, 0, 10)
            .await
            .expect("Failed to list");

        assert_eq!(list_result[0].src_url, src_url);
    }
}
