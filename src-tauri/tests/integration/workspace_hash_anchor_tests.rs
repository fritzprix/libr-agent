#![cfg(feature = "workspace-edit-file")]

use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::mcp::builtin::workspace::file_operations::utils::format_as_hashlines;
use tauri_mcp_agent_lib::mcp::builtin::workspace::tools::file_tools::create_edit_file_input_schema;
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

fn extract_anchor(hashlines: &str, line_index: usize) -> String {
    hashlines
        .lines()
        .nth(line_index)
        .and_then(|line| line.split('|').next())
        .and_then(|prefix| prefix.split(':').nth(1))
        .expect("anchor")
        .to_string()
}

fn line_ref(hashlines: &str, line_number: usize) -> String {
    let anchor = extract_anchor(hashlines, line_number - 1);
    format!("{line_number}:{anchor}")
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
fn edit_file_schema_uses_flat_start_anchor_variants() {
    let schema_json =
        serde_json::to_value(create_edit_file_input_schema()).expect("serialize editFile schema");
    let one_of = schema_json
        .get("oneOf")
        .and_then(|value| value.as_array())
        .expect("root oneOf array");
    assert_eq!(
        one_of.len(),
        3,
        "expected prepend, line-edit, and insert-after variants"
    );

    let prepend_variant = &one_of[0];
    let prepend_required = prepend_variant
        .get("required")
        .and_then(|value| value.as_array())
        .expect("prepend required");
    assert!(prepend_required
        .iter()
        .any(|value| value.as_str() == Some("path")));
    assert!(prepend_required
        .iter()
        .any(|value| value.as_str() == Some("content")));
    assert!(
        !prepend_required
            .iter()
            .any(|value| value.as_str() == Some("start")),
        "prepend variant should not require start"
    );

    let line_edit_variant = &one_of[1];
    let line_edit_required = line_edit_variant
        .get("required")
        .and_then(|value| value.as_array())
        .expect("line edit required");
    assert!(line_edit_required
        .iter()
        .any(|value| value.as_str() == Some("path")));
    assert!(line_edit_required
        .iter()
        .any(|value| value.as_str() == Some("start")));
    assert!(
        line_edit_variant
            .get("properties")
            .and_then(|properties| properties.get("start"))
            .is_some(),
        "line-edit variant should expose start"
    );
    assert!(
        line_edit_variant
            .get("properties")
            .and_then(|properties| properties.get("end"))
            .is_some(),
        "line-edit variant should expose end"
    );
    assert!(
        !schema_json
            .get("properties")
            .and_then(|properties| properties.get("edits"))
            .is_some(),
        "discovery schema should not expose edits array"
    );

    let insert_after_variant = &one_of[2];
    let insert_after_required = insert_after_variant
        .get("required")
        .and_then(|value| value.as_array())
        .expect("insert_after required");
    assert!(insert_after_required
        .iter()
        .any(|value| value.as_str() == Some("op")));
    assert!(insert_after_required
        .iter()
        .any(|value| value.as_str() == Some("start")));

    let start_description = line_edit_variant
        .get("properties")
        .and_then(|properties| properties.get("start"))
        .and_then(|value| value.get("description"))
        .and_then(|value| value.as_str())
        .expect("start description");
    assert!(
        start_description.contains("42:a31f2c")
            || start_description.contains("N:anchor")
            || start_description.contains("|content"),
        "start description should map to readFile line format: {start_description}"
    );
}

#[tokio::test]
async fn edit_file_flat_start_ref_replaces_single_line() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-file-flat-start-ref";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\nbeta\n").expect("write sample file");
    let start = line_ref(&format_as_hashlines("alpha\nbeta\n"), 1);

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "start": start,
                "content": "ALPHA"
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("flat edit should succeed");

    assert_eq!(result.is_error, Some(false));
    let updated = std::fs::read_to_string(workspace_dir.join("sample.txt")).expect("read updated");
    assert_eq!(updated, "ALPHA\nbeta\n");
}

#[tokio::test]
async fn edit_file_flat_start_and_end_replace_range() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-file-flat-start-end";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "a\nb\nc\nd\n").expect("write sample file");
    let hashlines = format_as_hashlines("a\nb\nc\nd\n");

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "start": line_ref(&hashlines, 2),
                "end": line_ref(&hashlines, 3),
                "content": "BC"
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("flat range edit should succeed");

    assert_eq!(result.is_error, Some(false));
    let updated = std::fs::read_to_string(workspace_dir.join("sample.txt")).expect("read updated");
    assert_eq!(updated, "a\nBC\nd\n");
}

#[tokio::test]
async fn edit_file_rejects_hashless_replace_of_existing_line() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
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
        text.contains("requires 'anchor'"),
        "expected missing-anchor error, got: {text}"
    );
}

#[tokio::test]
async fn edit_file_allows_hashless_insert_at_top() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
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
async fn edit_file_missing_target_reports_file_not_found_before_anchor_guidance() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-file-missing-target";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let result = server
        .handle_edit_file(
            json!({
                "path": "missing.txt",
                "edits": [
                    {
                        "op": "replace",
                        "startLine": 1,
                        "content": "hello"
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
        text.contains("does not exist"),
        "expected missing-file guidance, got: {text}"
    );
    assert!(
        !text.contains("requires 'anchor'"),
        "path errors should be reported before anchor guidance: {text}"
    );
}

#[tokio::test]
async fn edit_file_directory_target_reports_directory_error_before_anchor_guidance() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-file-directory-target";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::create_dir_all(workspace_dir.join("sample-dir")).expect("create sample dir");

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample-dir",
                "edits": [
                    {
                        "op": "replace",
                        "startLine": 1,
                        "content": "hello"
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
        text.contains("is a directory, not a file"),
        "expected directory-specific guidance, got: {text}"
    );
    assert!(
        text.contains("Use workspace__listDirectory"),
        "directory guidance should point at listDirectory: {text}"
    );
}

#[tokio::test]
async fn edit_file_ignores_anchors_for_top_insert() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-file-top-insert-anchor";
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
                        "startAnchor": "abcdef",
                        "content": "header"
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("edit should return MCPResult");

    assert_eq!(result.is_error, Some(false));
    let updated = std::fs::read_to_string(workspace_dir.join("sample.txt")).expect("read updated");
    assert_eq!(updated, "header\nalpha\nbeta\n");
}

#[tokio::test]
async fn edit_file_rejects_start_line_zero_for_replace_and_delete() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-file-rejects-zero-start-line";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\nbeta\n").expect("write sample file");

    for op in ["replace", "delete"] {
        let mut edit = json!({
            "op": op,
            "startLine": 0,
        });
        if op == "replace" {
            edit["content"] = json!("HEADER");
        }

        let result = server
            .handle_edit_file(
                json!({
                "path": "sample.txt",
                "edits": [edit]
                }),
                Some(session_id.to_string()),
            )
            .await
            .expect("edit should return MCPResult");

        let text = extract_text_content(&result);
        assert_eq!(result.is_error, Some(true));
        assert!(
            text.contains("'start' must be a 1-based")
                || text.contains("'startLine' must be >= 1")
                || text.contains("do not match the declared schema"),
            "expected invalid startLine guidance for op={op}, got: {text}"
        );
    }
}

#[tokio::test]
async fn edit_files_preserve_replace_and_insert_order_with_single_file_batch() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
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
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "edits": [
                    {
                        "op": "replace",
                        "startLine": 1,
                        "startAnchor": first_anchor,
                        "content": "A"
                    },
                    {
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
async fn edit_files_allows_replace_without_explicit_op() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-files-infers-replace";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\nbeta\n").expect("write sample file");

    let anchors = format_as_hashlines("alpha\nbeta\n");
    let first_anchor = anchors
        .lines()
        .next()
        .and_then(|line| line.split('|').next())
        .and_then(|prefix| prefix.split(':').nth(1))
        .expect("first anchor");

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "edits": [
                    {
                        "startLine": 1,
                        "startAnchor": first_anchor,
                        "content": "ALPHA"
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("edit batch should succeed");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(
        text.contains("Anchors above reflect the edited ranges"),
        "success response should explain anchor reuse without rereading: {text}"
    );
    let updated = std::fs::read_to_string(workspace_dir.join("sample.txt")).expect("read updated");
    assert_eq!(updated, "ALPHA\nbeta\n");
}

#[tokio::test]
async fn edit_files_delete_only_response_does_not_claim_new_anchors_exist() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-files-delete-only-guidance";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\nbeta\ngamma\n")
        .expect("write sample file");

    let anchors = format_as_hashlines("alpha\nbeta\ngamma\n");
    let second_anchor = anchors
        .lines()
        .nth(1)
        .and_then(|line| line.split('|').next())
        .and_then(|prefix| prefix.split(':').nth(1))
        .expect("second anchor");

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "edits": [
                    {
                        "op": "delete",
                        "startLine": 2,
                        "startAnchor": second_anchor
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("edit batch should succeed");

    let text = extract_text_content(&result);
    assert!(
        text.contains(
            "no new anchors were generated because these edits only removed existing lines"
        ),
        "delete-only success should explain why there are no fresh anchors: {text}"
    );
    assert!(
        !text.contains("New anchors:\n```\n\n```"),
        "delete-only success should not render an empty new-anchors block: {text}"
    );
    assert!(
        !text.contains("Anchors above are current for the edited ranges"),
        "delete-only success must not claim that new anchors are available: {text}"
    );
}

#[tokio::test]
async fn edit_files_success_response_includes_diff_block() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-files-success-diff-anchors";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\nbeta\n").expect("write sample file");

    let original_hashlines = format_as_hashlines("alpha\nbeta\n");
    let first_anchor = extract_anchor(&original_hashlines, 0);

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "edits": [
                    {
                        "op": "replace",
                        "startLine": 1,
                        "startAnchor": first_anchor,
                        "content": "ALPHA"
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("edit batch should succeed");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(
        text.contains("Diff:\n```diff"),
        "success response should include a diff block: {text}"
    );
    assert!(
        text.contains("- alpha"),
        "diff should show removed line content without hashline annotations: {text}"
    );
    assert!(
        text.contains("+ ALPHA"),
        "diff should show added line content without hashline annotations: {text}"
    );
    assert!(
        text.contains("  beta"),
        "diff should render unchanged context lines as plain text: {text}"
    );
}

#[tokio::test]
async fn edit_files_delete_only_response_includes_diff_block() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-files-delete-only-diff";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "alpha\nbeta\ngamma\n")
        .expect("write sample file");

    let original_hashlines = format_as_hashlines("alpha\nbeta\ngamma\n");
    let second_anchor = extract_anchor(&original_hashlines, 1);

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "edits": [
                    {
                        "op": "delete",
                        "startLine": 2,
                        "startAnchor": second_anchor,
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("edit batch should succeed");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(
        text.contains("Diff:\n```diff"),
        "delete-only success should still include a diff block: {text}"
    );
    assert!(
        text.contains("- beta"),
        "delete-only diff should show removed line content without hashline annotations: {text}"
    );
    assert!(
        text.contains("  alpha") && text.contains("  gamma"),
        "delete-only diff should keep surrounding context lines as plain text: {text}"
    );
}

#[tokio::test]
async fn edit_files_diff_preview_truncates_large_changes() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-files-diff-preview-truncation";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    let original_content = (1..=220)
        .map(|idx| format!("old-{idx:03}-{}", "x".repeat(480)))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let updated_content = (1..=220)
        .map(|idx| format!("new-{idx:03}-{}", "y".repeat(480)))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    std::fs::write(workspace_dir.join("sample.txt"), &original_content).expect("write sample file");

    let original_hashlines = format_as_hashlines(&original_content);
    let start_anchor = extract_anchor(&original_hashlines, 0);
    let end_anchor = extract_anchor(&original_hashlines, 219);

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "edits": [
                    {
                        "op": "replace",
                        "startLine": 1,
                        "endLine": 220,
                        "startAnchor": start_anchor,
                        "endAnchor": end_anchor,
                        "content": updated_content,
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("large edit batch should succeed");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(
        text.contains("more diff line(s) omitted"),
        "large diffs should be truncated instead of dumping the full file: {text}"
    );
}

#[tokio::test]
async fn edit_files_diff_preview_omitted_count_ignores_gap_markers() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-files-diff-preview-gap-marker-count";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    let target_lines = [2usize, 7, 12, 17, 22, 27, 32, 37, 42, 47, 52];
    let original_content = (1..=60)
        .map(|idx| format!("line-{idx:02}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    std::fs::write(workspace_dir.join("sample.txt"), &original_content).expect("write sample file");

    let original_hashlines = format_as_hashlines(&original_content);
    let anchor_lines: Vec<&str> = original_hashlines.lines().collect();
    let edits = target_lines
        .iter()
        .map(|line_number| {
            let start_anchor = anchor_lines[line_number - 1]
                .split('|')
                .next()
                .and_then(|prefix| prefix.split(':').nth(1))
                .expect("start anchor");

            json!({
                "op": "replace",
                "startLine": line_number,
                "startAnchor": start_anchor,
                "content": format!("updated-{line_number:02}"),
            })
        })
        .collect::<Vec<_>>();

    let result = server
        .handle_edit_file(
            json!({ "path": "sample.txt", "edits": edits }),
            Some(session_id.to_string()),
        )
        .await
        .expect("edit batch should succeed");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(
        text.contains("  ... 4 more diff line(s) omitted"),
        "truncation should count only omitted diff lines, not omitted-gap markers: {text}"
    );
    assert!(
        !text.contains("  ... 5 more diff line(s) omitted"),
        "gap markers must not inflate omitted diff line counts: {text}"
    );
}

#[tokio::test]
async fn edit_files_delete_range_at_eof_does_not_panic() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-files-delete-range-eof";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(
        workspace_dir.join("sample.txt"),
        "alpha\nbeta\ngamma\ndelta\n",
    )
    .expect("write sample file");

    let anchors = format_as_hashlines("alpha\nbeta\ngamma\ndelta\n");
    let anchor_lines: Vec<&str> = anchors.lines().collect();
    let start_anchor = anchor_lines[2]
        .split('|')
        .next()
        .and_then(|prefix| prefix.split(':').nth(1))
        .expect("start anchor");
    let end_anchor = anchor_lines[3]
        .split('|')
        .next()
        .and_then(|prefix| prefix.split(':').nth(1))
        .expect("end anchor");

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "edits": [
                    {
                        "op": "delete",
                        "startLine": 3,
                        "endLine": 4,
                        "startAnchor": start_anchor,
                        "endAnchor": end_anchor
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("delete range at EOF should return MCPResult");

    assert_eq!(result.is_error, Some(false));
    let text = extract_text_content(&result);
    assert!(
        text.contains(
            "no new anchors were generated because these edits only removed existing lines"
        ),
        "delete-only range at EOF should explain missing anchors: {text}"
    );
    let updated = std::fs::read_to_string(workspace_dir.join("sample.txt")).expect("read updated");
    assert_eq!(updated, "alpha\nbeta\n");
}

#[tokio::test]
async fn edit_files_error_messages_include_edit_context() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-files-error-context";
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
                        "startLine": 1,
                        "content": "new line"
                    }
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("edit batch should return MCPResult");

    assert_eq!(result.is_error, Some(true));
    let text = extract_text_content(&result);
    assert!(
        text.contains("Edit at index 0 [op='insert_after', startLine=1]")
            && text.contains("requires 'anchor'"),
        "error should include offending edit context: {text}"
    );
}

#[tokio::test]
async fn edit_file_rejects_multiline_replace_without_end_hash() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
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
        text.contains("requires 'end'") || text.contains("requires 'endAnchor'"),
        "expected missing end/endAnchor error, got: {text}"
    );
}

#[tokio::test]
async fn edit_file_allows_multiline_replace_with_end_anchor() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
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
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
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
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
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
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
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
async fn edit_file_rejects_stale_anchor_without_partial_write() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-file-stale-anchor";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("b.txt"), "one\ntwo\n").expect("write b");

    let result = server
        .handle_edit_file(
            json!({
                "path": "b.txt",
                "edits": [
                    {
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
        .expect("editFile should return MCPResult");

    assert_eq!(result.is_error, Some(true));
    let text = extract_text_content(&result);
    assert!(
        text.contains("STALE ANCHOR"),
        "expected stale anchor error, got: {text}"
    );
    assert!(
        text.contains("edit #1"),
        "stale anchor error should identify the edit index, got: {text}"
    );
    assert_eq!(
        std::fs::read_to_string(workspace_dir.join("b.txt")).expect("read b"),
        "one\ntwo\n"
    );
}

use super::env_var_guard::EnvVarGuard;

#[tokio::test]
async fn edit_file_rejects_oversized_target_file() {
    let _env_guard = EnvVarGuard::set_temp("LIBRAGENT_MAX_FILE_SIZE", "10");
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-file-size-limit";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("large.txt"), "01234567890123456789").expect("write file");

    let result = server
        .handle_edit_file(
            json!({
                "path": "large.txt",
                "edits": [{ "content": "x" }]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("editFile should return");

    assert!(
        result.is_error.unwrap_or(false),
        "oversized file should fail, got: {:?}",
        result
    );
    let text = extract_text_content(&result);
    assert!(
        text.contains("exceeds the maximum allowed size") || text.contains("File size error"),
        "expected size-limit error, got: {text}"
    );
}

#[tokio::test]
async fn edit_file_rejects_more_than_max_edits() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-file-max-edits";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "line\n").expect("write sample file");

    let edits: Vec<serde_json::Value> = (0..51).map(|_| json!({ "content": "x" })).collect();

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "edits": edits
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("editFile should return");

    assert!(
        result.is_error.unwrap_or(false),
        "more than 50 edits should fail, got: {:?}",
        result
    );
    let text = extract_text_content(&result);
    assert!(
        text.contains("exceeds the maximum of 50") || text.contains("more than 50 items"),
        "expected max-edits error, got: {text}"
    );
}

#[tokio::test]
async fn edit_file_content_only_prepends_without_start_line() {
    let _env_test_read_scope = super::env_var_guard::EnvTestReadGuard::acquire();
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-file-content-prepend";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("sample.txt"), "body\n").expect("write sample file");

    let result = server
        .handle_edit_file(
            json!({
                "path": "sample.txt",
                "edits": [{ "content": "header\n" }]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("editFile should return");

    assert!(
        !result.is_error.unwrap_or(true),
        "content-only prepend should succeed, got: {:?}",
        result
    );
    assert_eq!(
        std::fs::read_to_string(workspace_dir.join("sample.txt")).expect("read sample"),
        "header\nbody\n"
    );
}
