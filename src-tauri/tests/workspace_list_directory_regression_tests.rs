#![cfg(windows)]

use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
use tauri_mcp_agent_lib::session::SessionManager;
use tempfile::tempdir;

fn build_workspace_server(base_dir: &Path, session_id: &str) -> WorkspaceServer {
    let session_manager =
        SessionManager::new_with_base_dir(base_dir.to_path_buf()).expect("session manager");
    WorkspaceServer::new(session_id.to_string(), Arc::new(session_manager))
}

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

#[tokio::test]
async fn list_directory_returns_not_found_contract_for_missing_workspace_subdirectory_on_windows() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "list-dir-missing-windows";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let result = server
        .handle_list_directory(
            json!({
                "path": "skills/ai-daily-analyst/assets"
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("listDirectory should return MCPResult");

    let text = extract_text_content(&result);
    assert_eq!(result.is_error, Some(true));
    assert!(
        text.contains("Directory 'skills/ai-daily-analyst/assets' not found"),
        "missing directory should use the not-found contract: {text}"
    );
    assert!(
        text.contains("Use listDirectory to see available files"),
        "resource-not-found guidance should stay visible: {text}"
    );
    assert!(
        !text.contains("Verify the directory exists"),
        "generic operation-failed guidance would mean localized not-found detection regressed: {text}"
    );
}

#[tokio::test]
async fn list_directory_accepts_plain_and_dot_prefixed_relative_paths_on_windows() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "list-dir-dot-prefix-windows";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);
    let assets_dir = workspace_dir.join("skills/ai-daily-analyst/assets");

    std::fs::create_dir_all(&assets_dir).expect("assets dir");
    std::fs::write(assets_dir.join("brief.txt"), "hello").expect("asset file");

    let plain_result = server
        .handle_list_directory(
            json!({
                "path": "skills/ai-daily-analyst/assets"
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("plain relative listDirectory should succeed");

    let dot_prefixed_result = server
        .handle_list_directory(
            json!({
                "path": "./skills/ai-daily-analyst/assets"
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("dot-prefixed relative listDirectory should succeed");

    assert_eq!(plain_result.is_error, Some(false));
    assert_eq!(dot_prefixed_result.is_error, Some(false));

    let plain_structured = plain_result
        .structured_content
        .as_ref()
        .expect("plain structured content expected");
    let dot_prefixed_structured = dot_prefixed_result
        .structured_content
        .as_ref()
        .expect("dot-prefixed structured content expected");

    assert_eq!(plain_structured["items"], dot_prefixed_structured["items"]);
    assert_eq!(plain_structured["count"], json!(1));
    assert_eq!(dot_prefixed_structured["count"], json!(1));

    let plain_text = extract_text_content(&plain_result);
    let dot_prefixed_text = extract_text_content(&dot_prefixed_result);
    assert!(
        plain_text.contains("brief.txt"),
        "plain relative path should list the expected file: {plain_text}"
    );
    assert!(
        dot_prefixed_text.contains("brief.txt"),
        "dot-prefixed relative path should list the expected file: {dot_prefixed_text}"
    );
}

#[tokio::test]
async fn list_directory_allows_double_dots_inside_path_components_on_windows() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "list-dir-double-dot-component-windows";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);
    let assets_dir = workspace_dir.join("logs..old/assets");

    std::fs::create_dir_all(&assets_dir).expect("assets dir");
    std::fs::write(assets_dir.join("brief.txt"), "hello").expect("asset file");

    let result = server
        .handle_list_directory(
            json!({
                "path": "logs..old/assets"
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("listDirectory should allow legitimate path components containing double dots");

    assert_eq!(result.is_error, Some(false));
    assert_eq!(
        result
            .structured_content
            .as_ref()
            .expect("structured content expected")["count"],
        json!(1)
    );

    let text = extract_text_content(&result);
    assert!(
        text.contains("brief.txt"),
        "directory names containing '..' should still list their contents: {text}"
    );
}
