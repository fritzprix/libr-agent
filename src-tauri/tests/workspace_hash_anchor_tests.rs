use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::workspace::file_operations::utils::format_as_hashlines;
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

#[test]
fn anchored_line_format_uses_single_opaque_anchor() {
    let rendered = format_as_hashlines("alpha\nbeta");
    let first_line = rendered.lines().next().unwrap();
    let parts: Vec<&str> = first_line.splitn(2, '|').collect();
    assert_eq!(parts.len(), 2);

    let line_prefix = parts[0];
    let first_colon = line_prefix.find(':').unwrap();
    let anchor = &line_prefix[first_colon + 1..];

    assert_eq!(anchor.len(), 6);
    assert!(anchor.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn later_prefix_hash_changes_when_earlier_content_changes() {
    let original = format_as_hashlines("alpha\nbeta\ngamma");
    let changed = format_as_hashlines("alpha changed\nbeta\ngamma");

    let original_third = original.lines().nth(2).unwrap();
    let changed_third = changed.lines().nth(2).unwrap();

    assert_ne!(original_third, changed_third);
}

#[tokio::test]
async fn edit_file_rejects_hashless_replace_of_existing_line() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-file-requires-hashes";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\nbeta\n").expect("write sample file");

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "edits": [
                    {
                        "line": 1,
                        "action": "REPLACE",
                        "new_value": "ALPHA"
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("edit should return MCPResult");

    let text = extract_text_content(&result);
    assert_eq!(result.is_error, Some(true));
    assert!(
        text.contains("requires 'anchor'"),
        "expected missing-anchor error, got: {text}"
    );
}

#[tokio::test]
async fn edit_file_allows_hashless_insert_at_top() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-file-allows-top-insert";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\nbeta\n").expect("write sample file");

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "edits": [
                    {
                        "line": 0,
                        "action": "INSERT_AFTER",
                        "new_value": "header"
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("edit should succeed");

    assert_eq!(result.is_error, Some(false));
    let updated = std::fs::read_to_string(workspace_dir.join("sample.txt")).expect("read updated");
    assert_eq!(updated, "header\nalpha\nbeta\n");
}

#[tokio::test]
async fn edit_file_rejects_multiline_replace_without_end_hash() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-file-multiline-needs-end-hash";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\nbeta\ngamma\n")
        .expect("write sample file");

    let anchors = format_as_hashlines("alpha\nbeta\ngamma\n");
    let first_anchor = anchors.lines().next().expect("first anchor");
    let first_anchor_prefix = first_anchor.split('|').next().expect("anchor prefix");
    let anchor_parts: Vec<&str> = first_anchor_prefix.split(':').collect();
    assert_eq!(anchor_parts.len(), 2);

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "edits": [
                    {
                        "line": 1,
                        "endLine": 2,
                        "action": "REPLACE",
                        "anchor": anchor_parts[1],
                        "new_value": "ALPHA\nBETA"
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("edit should return MCPResult");

    assert_eq!(result.is_error, Some(true));
    let text = extract_text_content(&result);
    assert!(
        text.contains("requires 'endAnchor'"),
        "expected missing endAnchor error, got: {text}"
    );
}

#[tokio::test]
async fn edit_file_allows_multiline_replace_with_end_anchor() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-file-multiline-with-end-hash";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\nbeta\ngamma\n")
        .expect("write sample file");

    let anchors = format_as_hashlines("alpha\nbeta\ngamma\n");
    let anchor_lines: Vec<&str> = anchors.lines().collect();
    let start_parts: Vec<&str> = anchor_lines[0]
        .split('|')
        .next()
        .expect("start anchor prefix")
        .split(':')
        .collect();
    let end_parts: Vec<&str> = anchor_lines[1]
        .split('|')
        .next()
        .expect("end anchor prefix")
        .split(':')
        .collect();
    assert_eq!(start_parts.len(), 2);
    assert_eq!(end_parts.len(), 2);

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "edits": [
                    {
                        "line": 1,
                        "endLine": 2,
                        "action": "REPLACE",
                        "anchor": start_parts[1],
                        "endAnchor": end_parts[1],
                        "new_value": "ALPHA\nBETA"
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("edit should succeed");

    assert_eq!(result.is_error, Some(false));
    let updated = std::fs::read_to_string(workspace_dir.join("sample.txt")).expect("read updated");
    assert_eq!(updated, "ALPHA\nBETA\ngamma\n");
}
