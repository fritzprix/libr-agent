//! Windows-safe coverage for read-only tool next-action suppression.
//!
//! Keeps truncation coaching in the body while omitting edit-promotion
//! `💡 Next:` hints that previously padded every successful read/list.

#[path = "common/workspace_hint_assertions.rs"]
mod workspace_hint_assertions;

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
use tauri_mcp_agent_lib::session::SessionManager;
use tempfile::tempdir;
use workspace_hint_assertions::assert_no_edit_promotion_next_actions;

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
async fn read_file_success_omits_edit_promotion_hints() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-hint-suppress";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);
    std::fs::write(workspace_dir.join("note.txt"), "hello world\n").expect("write note.txt");

    let result = server
        .handle_read_file(json!({ "path": "note.txt" }), Some(session_id.to_string()))
        .await
        .expect("readFile should succeed");

    let text = extract_text_content(&result);
    assert!(
        text.contains("hello world"),
        "body should include file content: {text}"
    );
    assert_no_edit_promotion_next_actions(&text);
}

#[tokio::test]
async fn list_directory_success_omits_generic_next_actions() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "list-dir-hint-suppress";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);
    std::fs::create_dir_all(workspace_dir.join("empty")).expect("create empty dir");

    let result = server
        .handle_list_directory(json!({ "path": "empty" }), Some(session_id.to_string()))
        .await
        .expect("listDirectory should succeed");

    let text = extract_text_content(&result);
    assert!(
        text.contains("empty"),
        "body should describe the empty directory: {text}"
    );
    assert!(
        !text.contains("💡 Next:"),
        "listDirectory success must not append next-action hints: {text}"
    );
    assert!(
        !text.contains("writeFile"),
        "empty listDirectory must not promote writeFile: {text}"
    );
}
