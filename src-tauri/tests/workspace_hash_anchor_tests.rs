use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::workspace::file_operations::utils::format_as_hashlines;
use tauri_mcp_agent_lib::mcp::builtin::workspace::tools::file_tools::create_edit_files_input_schema;
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

#[test]
fn edit_files_schema_exposes_op_variants_via_one_of() {
    let schema_json =
        serde_json::to_value(create_edit_files_input_schema()).expect("serialize editFiles schema");
    let edits_items = schema_json
        .get("properties")
        .and_then(|properties| properties.get("edits"))
        .and_then(|edits| edits.get("items"))
        .expect("edits.items schema");
    let variants = edits_items
        .get("oneOf")
        .and_then(|value| value.as_array())
        .expect("oneOf variants");

    assert_eq!(variants.len(), 6, "expected replace/insert/delete variants");
    for variant in variants {
        let required = variant
            .get("required")
            .and_then(|value| value.as_array())
            .expect("variant required array");
        assert!(
            required.iter().any(|value| value.as_str() == Some("path")),
            "every editFiles variant should require path"
        );
    }
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
                        "op": "replace",
                        "startLine": 1,
                        "content": "ALPHA"
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
        text.contains("declared schema") && text.contains("startAnchor"),
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
                        "op": "insert_after",
                        "startLine": 0,
                        "content": "header"
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
async fn edit_files_preserve_replace_and_insert_order_with_single_file_batch() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-files-replace-insert-order";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "a\nb\nc\n").expect("write sample file");

    let anchors = format_as_hashlines("a\nb\nc\n");
    let anchor_lines: Vec<&str> = anchors.lines().collect();
    let first_anchor = anchor_lines[0]
        .split('|')
        .next()
        .and_then(|prefix| prefix.split(':').nth(1))
        .expect("first anchor");
    let second_anchor = anchor_lines[1]
        .split('|')
        .next()
        .and_then(|prefix| prefix.split(':').nth(1))
        .expect("second anchor");

    let result = server
        .handle_edit_files(
            json!({
                "edits": [
                    {
                        "path": "sample.txt",
                        "op": "replace",
                        "startLine": 1,
                        "startAnchor": first_anchor,
                        "content": "A"
                    },
                    {
                        "path": "sample.txt",
                        "op": "insert_after",
                        "startLine": 2,
                        "startAnchor": second_anchor,
                        "content": "B+"
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("edit batch should succeed");

    assert_eq!(result.is_error, Some(false));
    let updated = std::fs::read_to_string(workspace_dir.join("sample.txt")).expect("read updated");
    assert_eq!(updated, "A\nb\nB+\nc\n");
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
                        "op": "replace",
                        "startLine": 1,
                        "endLine": 2,
                        "startAnchor": anchor_parts[1],
                        "content": "ALPHA\nBETA"
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
        text.contains("declared schema") && text.contains("endAnchor"),
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
                        "op": "replace",
                        "startLine": 1,
                        "endLine": 2,
                        "startAnchor": start_parts[1],
                        "endAnchor": end_parts[1],
                        "content": "ALPHA\nBETA"
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

#[tokio::test]
async fn legacy_replace_lines_alias_still_routes_through_edit_file() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "legacy-replace-lines-alias";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\nbeta\n").expect("write sample file");

    let anchors = format_as_hashlines("alpha\nbeta\n");
    let start_anchor = anchors
        .lines()
        .next()
        .and_then(|line| line.split('|').next())
        .and_then(|prefix| prefix.split(':').nth(1))
        .expect("start anchor");

    let result = server
        .handle_replace_lines(
            json!({
                "path": "sample.txt",
                "line": 1,
                "anchor": start_anchor,
                "new_value": "ALPHA"
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("legacy alias should succeed");

    assert_eq!(result.is_error, Some(false));
    let updated = std::fs::read_to_string(workspace_dir.join("sample.txt")).expect("read updated");
    assert_eq!(updated, "ALPHA\nbeta\n");
}

#[tokio::test]
async fn legacy_edit_file_insert_after_flag_still_works() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "legacy-edit-file-insert-after-flag";
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
                        "insertAfter": true,
                        "new_value": "header"
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("legacy insertAfter edit should succeed");

    assert_eq!(result.is_error, Some(false));
    let updated = std::fs::read_to_string(workspace_dir.join("sample.txt")).expect("read updated");
    assert_eq!(updated, "header\nalpha\nbeta\n");
}

#[tokio::test]
async fn legacy_edit_file_empty_new_value_still_deletes() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "legacy-edit-file-empty-new-value-delete";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\nbeta\n").expect("write sample file");

    let anchors = format_as_hashlines("alpha\nbeta\n");
    let start_anchor = anchors
        .lines()
        .next()
        .and_then(|line| line.split('|').next())
        .and_then(|prefix| prefix.split(':').nth(1))
        .expect("start anchor");

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "edits": [
                    {
                        "line": 1,
                        "anchor": start_anchor,
                        "new_value": ""
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("legacy empty new_value delete should succeed");

    assert_eq!(result.is_error, Some(false));
    let updated = std::fs::read_to_string(workspace_dir.join("sample.txt")).expect("read updated");
    assert_eq!(updated, "beta\n");
}

#[tokio::test]
async fn edit_files_applies_multi_file_batch_atomically() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-files-multi-file";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("a.txt"), "alpha\nbeta\n").expect("write a");
    std::fs::write(workspace_dir.join("b.txt"), "one\ntwo\n").expect("write b");

    let a_anchors = format_as_hashlines("alpha\nbeta\n");
    let a_anchor = a_anchors
        .lines()
        .next()
        .and_then(|line| line.split('|').next())
        .and_then(|prefix| prefix.split(':').nth(1))
        .expect("a anchor");
    let b_anchors = format_as_hashlines("one\ntwo\n");
    let b_anchor = b_anchors
        .lines()
        .nth(1)
        .and_then(|line| line.split('|').next())
        .and_then(|prefix| prefix.split(':').nth(1))
        .expect("b anchor");

    let result = server
        .handle_edit_files(
            json!({
                "edits": [
                    {
                        "path": "a.txt",
                        "op": "replace",
                        "startLine": 1,
                        "startAnchor": a_anchor,
                        "content": "ALPHA"
                    },
                    {
                        "path": "b.txt",
                        "op": "delete",
                        "startLine": 2,
                        "startAnchor": b_anchor
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("multi-file edit should succeed");

    assert_eq!(result.is_error, Some(false));
    assert_eq!(
        std::fs::read_to_string(workspace_dir.join("a.txt")).expect("read a"),
        "ALPHA\nbeta\n"
    );
    assert_eq!(
        std::fs::read_to_string(workspace_dir.join("b.txt")).expect("read b"),
        "one\n"
    );
}

#[tokio::test]
async fn edit_files_rejects_stale_anchor_without_partial_write() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-files-rollback";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("a.txt"), "alpha\nbeta\n").expect("write a");
    std::fs::write(workspace_dir.join("b.txt"), "one\ntwo\n").expect("write b");

    let a_anchors = format_as_hashlines("alpha\nbeta\n");
    let a_anchor = a_anchors
        .lines()
        .next()
        .and_then(|line| line.split('|').next())
        .and_then(|prefix| prefix.split(':').nth(1))
        .expect("a anchor");

    let result = server
        .handle_edit_files(
            json!({
                "edits": [
                    {
                        "path": "a.txt",
                        "op": "replace",
                        "startLine": 1,
                        "startAnchor": a_anchor,
                        "content": "ALPHA"
                    },
                    {
                        "path": "b.txt",
                        "op": "replace",
                        "startLine": 1,
                        "startAnchor": "deadbe",
                        "content": "ONE"
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("multi-file edit should return MCPResult");

    assert_eq!(result.is_error, Some(true));
    let text = extract_text_content(&result);
    assert!(
        text.contains("STALE ANCHOR"),
        "expected stale anchor error, got: {text}"
    );
    assert_eq!(
        std::fs::read_to_string(workspace_dir.join("a.txt")).expect("read a"),
        "alpha\nbeta\n"
    );
    assert_eq!(
        std::fs::read_to_string(workspace_dir.join("b.txt")).expect("read b"),
        "one\ntwo\n"
    );
}
