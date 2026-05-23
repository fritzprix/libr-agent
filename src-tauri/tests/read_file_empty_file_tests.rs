mod common;

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
async fn read_file_allows_explicit_start_line_one_for_empty_file() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-empty-file-start-line-one";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("empty.txt"), "").expect("write empty file");

    let result = server
        .handle_read_file(
            json!({
                "path": "empty.txt",
                "startLine": 1
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readFile should return MCPResult");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(
        text.contains("no lines shown"),
        "startLine=1 should still succeed for empty files: {text}"
    );
}

#[tokio::test]
async fn read_file_rejects_zero_start_line_without_end_line() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-zero-start-line";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\n").expect("write file");

    let result = server
        .handle_read_file(
            json!({
                "path": "sample.txt",
                "startLine": 0
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readFile should return MCPResult");

    assert_eq!(result.is_error, Some(true));
    let text = extract_text_content(&result);
    assert!(
        text.contains("Line numbers must be >= 1"),
        "startLine=0 should be rejected consistently: {text}"
    );
}

#[tokio::test]
async fn read_file_rejects_non_integer_numeric_start_line() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-decimal-start-line";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\nbeta\n").expect("write file");

    let result = server
        .handle_read_file(
            json!({
                "path": "sample.txt",
                "startLine": 1.5
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readFile should return MCPResult");

    assert_eq!(result.is_error, Some(true));
    let text = extract_text_content(&result);
    assert!(
        text.contains("startLine must be a positive integer"),
        "non-integer line bounds should be rejected instead of treated as missing: {text}"
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
                "startLine": 5
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
        !text.contains("Choose startLine between 1 and 0"),
        "empty-file guidance should not suggest an impossible range: {text}"
    );
    assert!(
        text.contains("use startLine: 1"),
        "empty-file guidance should point callers at the only valid explicit line bound: {text}"
    );
}
