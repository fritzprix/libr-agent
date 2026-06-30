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
async fn read_file_returns_empty_content_for_empty_file() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-empty-file";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("empty.txt"), "").expect("write empty file");

    let result = server
        .handle_read_file(json!({ "path": "empty.txt" }), Some(session_id.to_string()))
        .await
        .expect("readFile should return MCPResult");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(
        text.contains("no lines shown"),
        "empty file reads should succeed with an empty-file summary: {text}"
    );
}

#[tokio::test]
async fn read_file_allows_explicit_offset_one_for_empty_file() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-empty-file-offset-one";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("empty.txt"), "").expect("write empty file");

    let result = server
        .handle_read_file(
            json!({
                "path": "empty.txt",
                "offset": 1
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readFile should return MCPResult");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(
        text.contains("no lines shown"),
        "offset=1 should still succeed for empty files: {text}"
    );
}

#[tokio::test]
async fn read_file_rejects_zero_size() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-zero-size";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\n").expect("write file");

    let result = server
        .handle_read_file(
            json!({
                "path": "sample.txt",
                "size": 0
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readFile should return MCPResult");

    assert_eq!(result.is_error, Some(true));
    let text = extract_text_content(&result);
    assert!(
        text.contains("size must be non-zero"),
        "size=0 should be rejected consistently: {text}"
    );
}

#[tokio::test]
async fn read_file_rejects_non_integer_numeric_offset() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-decimal-offset";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\nbeta\n").expect("write file");

    let result = server
        .handle_read_file(
            json!({
                "path": "sample.txt",
                "offset": 1.5
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readFile should return MCPResult");

    assert_eq!(result.is_error, Some(true));
    let text = extract_text_content(&result);
    assert!(
        text.contains("offset must be an integer"),
        "non-integer offset should be rejected: {text}"
    );
}

#[tokio::test]
async fn read_file_empty_file_out_of_range_has_empty_file_guidance() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-empty-out-of-range";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("empty.txt"), "").expect("write empty file");

    let result = server
        .handle_read_file(
            json!({
                "path": "empty.txt",
                "offset": 5
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readFile should return MCPResult");

    assert_eq!(result.is_error, Some(true));
    let text = extract_text_content(&result);
    assert!(
        text.contains("File is empty (0 lines)"),
        "empty-file out-of-range should say the file is empty: {text}"
    );
    assert!(
        text.contains("omit offset/size or use offset: 1"),
        "empty-file guidance should point callers at the only valid explicit offset: {text}"
    );
}
