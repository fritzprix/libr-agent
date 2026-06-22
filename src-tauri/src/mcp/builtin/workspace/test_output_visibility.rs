#[cfg(test)]
mod tests {
    use super::super::WorkspaceServer;
    use crate::entity::settings;
    use crate::session::SessionManager;
    use sea_orm::{ConnectionTrait, Database, Schema};
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::time::{sleep, Duration};

    struct WorkspaceTestHarness {
        _guard: tokio::sync::MutexGuard<'static, ()>,
        server: WorkspaceServer,
    }

    async fn create_server() -> WorkspaceTestHarness {
        let guard = crate::state::lock_test_global_state().await;
        crate::state::reset_state();

        let temp_dir = tempdir().unwrap();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to create settings database");
        let schema = Schema::new(db.get_database_backend());
        let stmt = schema.create_table_from_entity(settings::Entity);
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create settings table");
        crate::lifecycle::repositories::init_repositories(&db).await;
        crate::state::init_session_bus(crate::agent::session_bus::SessionBus::new());
        crate::state::init_concurrency_gate(crate::agent::concurrency::ConcurrencyGate::new(
            crate::agent::concurrency::DEFAULT_MAX_ACTIVE_AGENTS,
            crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_AGENTS,
            crate::agent::concurrency::DEFAULT_MAX_ACTIVE_PROCESSES,
            crate::agent::concurrency::DEFAULT_MAX_SUSPENDED_PROCESSES,
        ));

        let session_manager =
            Arc::new(SessionManager::new_with_base_dir(temp_dir.path().to_path_buf()).unwrap());
        WorkspaceTestHarness {
            _guard: guard,
            server: WorkspaceServer::new("test-session".to_string(), session_manager),
        }
    }

    #[tokio::test]
    async fn test_read_process_output_visibility() {
        let harness = create_server().await;
        let server = &harness.server;

        // Use handle_spawn_process directly
        let exec_args = json!({
            "command": "echo \"Hello World output line 1\nHello World output line 2\"",
        });

        let result = server
            .handle_spawn_process(exec_args, "test-session")
            .await
            .expect("Execution failed");
        let data = result.structured_content.expect("No data returned");
        let process_id = data["process_id"]
            .as_str()
            .expect("data has no process_id")
            .to_string();

        let mut finished = false;
        for _ in 0..20 {
            let poll_args = json!({ "processId": process_id, "timeout": 0 });
            let poll_res = server
                .call_tool(
                    "waitForProcess",
                    poll_args,
                    Some("test-session".to_string()),
                )
                .await
                .expect("Poll failed");
            let poll_data = poll_res.structured_content.as_ref().unwrap();
            let status = poll_data["status"].as_str().unwrap();

            if status == "finished" || status == "failed" {
                finished = true;
                break;
            }
            sleep(Duration::from_millis(200)).await;
        }
        assert!(finished, "Process did not finish in time");

        let read_args = json!({
            "processId": process_id,
            "stream": "stdout"
        });
        let read_res = server
            .call_tool(
                "readProcessOutput",
                read_args,
                Some("test-session".to_string()),
            )
            .await
            .expect("Read failed");

        let text_content = &read_res.content.unwrap()[0];
        match text_content {
            crate::mcp::types::MCPContent::Text { text, .. } => {
                println!("Result text: {}", text);
                assert!(text.contains("Hello World output line 1"));
                assert!(text.contains("Hello World output line 2"));
            }
            _ => panic!("Expected text content"),
        }
    }

    #[tokio::test]
    async fn test_poll_process_tail_visibility() {
        let harness = create_server().await;
        let server = &harness.server;

        // Use handle_spawn_process directly
        let exec_args = json!({
            "command": "echo \"Tail line 1\\nTail line 2\"",
        });

        let result = server
            .handle_spawn_process(exec_args, "test-session")
            .await
            .expect("Execution failed");
        let process_id = result.structured_content.unwrap()["process_id"]
            .as_str()
            .unwrap()
            .to_string();

        // Wait for finish
        let mut finished = false;
        for _ in 0..20 {
            let poll_args = json!({ "processId": process_id, "timeout": 0 });
            let poll_res = server
                .call_tool(
                    "waitForProcess",
                    poll_args,
                    Some("test-session".to_string()),
                )
                .await
                .expect("Poll failed");
            let poll_data = poll_res.structured_content.as_ref().unwrap();
            if poll_data["status"] == "finished" {
                finished = true;
                break;
            }
            sleep(Duration::from_millis(200)).await;
        }
        assert!(finished, "Process did not finish in time");

        let read_args = json!({
            "processId": process_id,
            "stream": "stdout",
            "mode": "tail",
            "lines": 5
        });

        let poll_res = server
            .call_tool(
                "readProcessOutput",
                read_args,
                Some("test-session".to_string()),
            )
            .await
            .expect("Poll failed");

        let text_content = &poll_res.content.unwrap()[0];
        match text_content {
            crate::mcp::types::MCPContent::Text { text, .. } => {
                println!("Poll Result text: {}", text);
                assert!(text.contains("Tail line 1"));
                assert!(text.contains("Tail line 2"));
                assert!(text
                    .contains("Internal output files (absolute paths, not workspace-relative):"));
                assert!(text.contains("[STDOUT]"));
            }
            _ => panic!("Expected text content"),
        }
    }
}
