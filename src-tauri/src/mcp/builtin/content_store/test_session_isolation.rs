#[cfg(test)]
mod tests {
    use crate::mcp::builtin::content_store::server::ContentStoreServer;
    use crate::mcp::types::ServiceContextOptions;
    use crate::session::SessionManager;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn setup_server(session_id: &str) -> (ContentStoreServer, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path().join(format!("session_{}", session_id));
        let db_path = base_dir.join("content_store.db");

        // Ensure database directory exists and create empty database file
        std::fs::create_dir_all(&base_dir).unwrap();
        std::fs::File::create(&db_path).unwrap();

        let url = format!("sqlite://{}", db_path.to_string_lossy());

        // Connect to database and run migrations
        let db = sea_orm::Database::connect(&url)
            .await
            .expect("Failed to connect to database");

        use migration::{Migrator, MigratorTrait};
        Migrator::up(&db, None)
            .await
            .expect("Failed to run migrations");

        let session_manager = Arc::new(
            SessionManager::new_with_base_dir(base_dir).expect("Failed to create SessionManager"),
        );
        let server =
            ContentStoreServer::new_with_db("test-session".to_string(), session_manager, db)
                .await
                .expect("Failed to create server");

        // Set session context before returning
        server
            .switch_context(ServiceContextOptions {
                session_id: Some(session_id.to_string()),
                assistant_id: None,
            })
            .await
            .expect("Failed to switch context");

        (server, temp_dir)
    }

    #[tokio::test]
    async fn test_read_content_cross_session_protection() {
        // Setup two separate sessions
        let (server_a, _temp_a) = setup_server("session-a").await;
        let (server_b, _temp_b) = setup_server("session-b").await;

        // Add content to session A
        let add_result = server_a
            .handle_save_knowledge(json!({
                "content": "Secret content from Session A",
                "metadata": {
                    "title": "Session A Document",
                    "filename": "secret.txt",
                    "mime_type": "text/plain"
                }
            }))
            .await
            .unwrap();

        // Extract content_id from the response
        let content_id = if let Some(structured) = add_result.structured_content {
            structured["contentId"].as_str().unwrap().to_string()
        } else {
            panic!("Failed to get content_id from add_content response");
        };

        // Verify session A can read its own content
        let read_result_a = server_a
            .handle_read_content(json!({
                "contentId": content_id.clone()
            }))
            .await
            .unwrap();

        assert_eq!(read_result_a.is_error, Some(false));
        // Check structured content for actual content
        let structured = read_result_a.structured_content.as_ref().unwrap();
        let actual_content = structured.get("content").unwrap().as_str().unwrap();
        assert!(actual_content.contains("Secret content from Session A"));

        // Attempt to read from session B (should fail with access denied)
        let read_result_b = server_b
            .handle_read_content(json!({
                "contentId": content_id
            }))
            .await
            .unwrap();

        // Should return error
        assert_eq!(read_result_b.is_error, Some(true));
        let error_vec = read_result_b.content.unwrap();
        let error_text = match &error_vec[0] {
            crate::mcp::types::MCPContent::Text { text } => text,
            _ => panic!("Expected text content"),
        };
        assert!(
            error_text.contains("Access denied") || error_text.contains("not found"),
            "Expected access denied or not found error, got: {}",
            error_text
        );
    }

    #[tokio::test]
    async fn test_content_isolation_between_sessions() {
        let (server_a, _temp_a) = setup_server("session-isolation-a").await;
        let (server_b, _temp_b) = setup_server("session-isolation-b").await;

        // Add content to session A
        server_a
            .handle_save_knowledge(json!({
                "content": "Session A content",
                "metadata": {
                    "title": "A Document",
                    "filename": "a.txt",
                    "mime_type": "text/plain"
                }
            }))
            .await
            .unwrap();

        // Add content to session B
        server_b
            .handle_save_knowledge(json!({
                "content": "Session B content",
                "metadata": {
                    "title": "B Document",
                    "filename": "b.txt",
                    "mime_type": "text/plain"
                }
            }))
            .await
            .unwrap();

        // List content in session A
        let list_a = server_a.handle_list_content(json!({})).await.unwrap();

        assert_eq!(list_a.is_error, Some(false));
        let structured_a = list_a.structured_content.unwrap();
        assert_eq!(structured_a["total"], 1);
        let items_a = structured_a["contents"].as_array().unwrap();
        assert_eq!(items_a.len(), 1);
        assert_eq!(items_a[0]["filename"], "a.txt");

        // List content in session B
        let list_b = server_b.handle_list_content(json!({})).await.unwrap();

        assert_eq!(list_b.is_error, Some(false));
        let structured_b = list_b.structured_content.unwrap();
        assert_eq!(structured_b["total"], 1);
        let items_b = structured_b["contents"].as_array().unwrap();
        assert_eq!(items_b.len(), 1);
        assert_eq!(items_b[0]["filename"], "b.txt");
    }

    #[tokio::test]
    async fn test_search_respects_session_boundaries() {
        let (server_a, _temp_a) = setup_server("session-search-a").await;
        let (server_b, _temp_b) = setup_server("session-search-b").await;

        // Add searchable content to session A
        server_a
            .handle_save_knowledge(json!({
                "content": "This document contains the keyword SEARCHTERM in session A",
                "metadata": {
                    "title": "Session A Searchable",
                    "filename": "searchable_a.txt",
                    "mime_type": "text/plain"
                }
            }))
            .await
            .unwrap();

        // Add different content to session B
        server_b
            .handle_save_knowledge(json!({
                "content": "This is a different document without the special keyword",
                "metadata": {
                    "title": "Session B Document",
                    "filename": "doc_b.txt",
                    "mime_type": "text/plain"
                }
            }))
            .await
            .unwrap();

        // Search in session A for SEARCHTERM
        let search_a = server_a
            .handle_search_knowledge(json!({
                "query": "SEARCHTERM"
            }))
            .await
            .unwrap();

        assert_eq!(search_a.is_error, Some(false));
        let structured_a = search_a.structured_content.unwrap();
        let results_a = structured_a["results"].as_array().unwrap();
        assert_eq!(results_a.len(), 1);

        // Search in session B for SEARCHTERM (should find nothing)
        let search_b = server_b
            .handle_search_knowledge(json!({
                "query": "SEARCHTERM"
            }))
            .await
            .unwrap();

        assert_eq!(search_b.is_error, Some(false));
        let structured_b = search_b.structured_content.unwrap();
        let results_b = structured_b["results"].as_array().unwrap();
        assert_eq!(
            results_b.len(),
            0,
            "Session B should not find Session A's content"
        );
    }

    #[tokio::test]
    async fn test_delete_content_cross_session_protection() {
        let (server_a, _temp_a) = setup_server("session-delete-a").await;
        let (server_b, _temp_b) = setup_server("session-delete-b").await;

        // Add content to session A
        let add_result = server_a
            .handle_save_knowledge(json!({
                "content": "Content to be protected from cross-session deletion",
                "metadata": {
                    "title": "Protected Document",
                    "filename": "protected.txt",
                    "mime_type": "text/plain"
                }
            }))
            .await
            .unwrap();

        let content_id = if let Some(structured) = add_result.structured_content {
            structured["contentId"].as_str().unwrap().to_string()
        } else {
            panic!("Failed to get content_id");
        };

        // Attempt to delete from session B (should fail)
        let delete_result_b = server_b
            .handle_delete_content(json!({
                "contentId": content_id.clone()
            }))
            .await
            .unwrap();

        // Should return error
        assert_eq!(delete_result_b.is_error, Some(true));
        let error_vec = delete_result_b.content.unwrap();
        let error_text = match &error_vec[0] {
            crate::mcp::types::MCPContent::Text { text } => text,
            _ => panic!("Expected text content"),
        };
        assert!(
            error_text.contains("Access denied") || error_text.contains("not found"),
            "Expected access denied or not found error, got: {}",
            error_text
        );

        // Verify content still exists in session A
        let list_a = server_a.handle_list_content(json!({})).await.unwrap();

        let structured_a = list_a.structured_content.unwrap();
        assert_eq!(structured_a["total"], 1);

        // Verify session A can still delete its own content
        let delete_result_a = server_a
            .handle_delete_content(json!({
                "contentId": content_id
            }))
            .await
            .unwrap();

        assert_eq!(delete_result_a.is_error, Some(false));

        // Verify deletion succeeded
        let list_after = server_a.handle_list_content(json!({})).await.unwrap();

        let structured_after = list_after.structured_content.unwrap();
        assert_eq!(structured_after["total"], 0);
    }
}
