#[cfg(test)]
mod tests {
    use super::super::WorkspaceServer;
    use crate::session::SessionManager;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::time::{sleep, Duration};

    async fn create_server() -> WorkspaceServer {
        let temp_dir = tempdir().unwrap();
        let session_manager =
            Arc::new(SessionManager::new_with_base_dir(temp_dir.path().to_path_buf()).unwrap());
        WorkspaceServer::new("test-session".to_string(), session_manager)
    }

    #[tokio::test]
    async fn test_read_process_output_visibility() {
        let server = create_server().await;

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
            let poll_args = json!({ "processId": process_id });
            let poll_res = server
                .handle_poll_process(poll_args, "test-session")
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
            .handle_read_process_output(read_args, "test-session")
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
        let server = create_server().await;

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
            let poll_args = json!({ "processId": process_id });
            let poll_res = server
                .handle_poll_process(poll_args, "test-session")
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

        let poll_args = json!({
            "processId": process_id,
            "tail": {
                "n": 5,
                "src": "stdout"
            }
        });

        let poll_res = server
            .handle_poll_process(poll_args, "test-session")
            .await
            .expect("Poll failed");

        let text_content = &poll_res.content.unwrap()[0];
        match text_content {
            crate::mcp::types::MCPContent::Text { text, .. } => {
                println!("Poll Result text: {}", text);
                assert!(text.contains("Tail line 1"));
                assert!(text.contains("Tail line 2"));
                assert!(text.contains("Output (last 2 lines)"));
            }
            _ => panic!("Expected text content"),
        }
    }
}
