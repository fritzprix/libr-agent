use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
use tauri_mcp_agent_lib::session::SessionManager;
use tempfile::tempdir;

fn extract_text_content(result: &MCPResult) -> String {
    result
        .content
        .as_ref()
        .expect("text content expected")
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_workspace_server(base_dir: &std::path::Path, session_id: &str) -> WorkspaceServer {
    let session_manager =
        SessionManager::new_with_base_dir(base_dir.to_path_buf()).expect("session manager");
    WorkspaceServer::new(session_id.to_string(), Arc::new(session_manager))
}

#[tokio::test]
async fn glob_files_works_correctly() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "glob-files-test";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    // Create test files
    std::fs::create_dir_all(workspace_dir.join("src")).expect("src dir");
    std::fs::create_dir_all(workspace_dir.join("tests")).expect("tests dir");
    std::fs::write(workspace_dir.join("src/main.rs"), "fn main() {}").expect("write main.rs");
    std::fs::write(workspace_dir.join("src/lib.rs"), "pub fn lib() {}").expect("write lib.rs");
    std::fs::write(workspace_dir.join("tests/integration.rs"), "mod tests;").expect("write integration.rs");
    std::fs::write(workspace_dir.join("README.md"), "# Project").expect("write README");

    // Call handle_glob_files directly (the new tool)
    let result = server
        .handle_glob_files(
            json!({
                "path": ".",
                "filePattern": "*.rs",
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("glob_files should succeed");

    let text = extract_text_content(&result);
    println!("GlobFiles output:\n{}", text);

    // Should find all .rs files
    assert!(text.contains("src/main.rs"), "Expected src/main.rs in output: {}", text);
    assert!(text.contains("src/lib.rs"), "Expected src/lib.rs in output: {}", text);
    assert!(text.contains("tests/integration.rs"), "Expected tests/integration.rs in output: {}", text);
    assert!(!text.contains("README.md"), "Should not include README.md: {}", text);

    // Check structured content
    let structured = result.structured_content.as_ref().expect("structured content expected");
    assert_eq!(structured["files_found"], json!(3));
}

#[tokio::test]
async fn grep_files_works_correctly() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "grep-files-test";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    // Create test files
    std::fs::create_dir_all(workspace_dir.join("src")).expect("src dir");
    std::fs::write(
        workspace_dir.join("src/main.rs"),
        "fn main() {\n    let x = 1;\n    let y = 2;\n}",
    )
    .expect("write main.rs");
    std::fs::write(
        workspace_dir.join("src/lib.rs"),
        "pub fn helper() -> i32 {\n    42\n}",
    )
    .expect("write lib.rs");

    // Call handle_grep_files directly (the new tool)
    let result = server
        .handle_grep_files(
            json!({
                "path": ".",
                "query": "let\\s+x\\s*=\\s*1",
                "filePattern": "*.rs",
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("grep_files should succeed");

    let text = extract_text_content(&result);
    println!("GrepFiles output:\n{}", text);

    // Should find the match in main.rs
    assert!(text.contains("src/main.rs"), "Expected src/main.rs in output: {}", text);
    assert!(text.contains("let x = 1;"), "Expected 'let x = 1;' in output: {}", text);
    assert!(!text.contains("src/lib.rs"), "Should not include lib.rs: {}", text);

    // Check structured content
    let structured = result.structured_content.as_ref().expect("structured content expected");
    assert_eq!(structured["total_matches"], json!(1));
}

#[tokio::test]
async fn glob_files_with_limit_and_offset() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "glob-files-pagination-test";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    // Create multiple test files
    for i in 0..10 {
        std::fs::write(workspace_dir.join(format!("file_{}.txt", i)), "content").expect("write file");
    }

    // Test limit
    let result = server
        .handle_glob_files(
            json!({
                "path": ".",
                "filePattern": "*.txt",
                "limit": 5,
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("glob_files should succeed");

    let text = extract_text_content(&result);
    assert!(text.contains("Showing 1 to 5 of 10 total"), "Expected pagination info: {}", text);
    assert_eq!(result.structured_content.as_ref().unwrap()["files_found"], json!(10));
}

#[tokio::test]
async fn grep_files_pagination_works() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "grep-files-pagination-test";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    // Create file with many matches
    let content = (0..20).map(|i| format!("line {} with needle", i)).collect::<Vec<_>>().join("\n");
    std::fs::write(workspace_dir.join("matches.txt"), content).expect("write file");

    // Test limit
    let result = server
        .handle_grep_files(
            json!({
                "path": ".",
                "query": "needle",
                "limit": 5,
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("grep_files should succeed");

    let text = extract_text_content(&result);
    assert!(text.contains("Showing 1 to 5 of 20 total matches"), "Expected pagination: {}", text);
    assert_eq!(result.structured_content.as_ref().unwrap()["total_matches"], json!(20));
}
