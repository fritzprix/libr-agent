use serde_json::json;
use std::sync::Arc;
use tauri_mcp_agent_lib::agent::tools::TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES;
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

fn seed_large_file(workspace_dir: &std::path::Path, path: &str) {
    let content = (1..=2_000)
        .map(|line_number| {
            format!(
                "line {line_number:04}: {}",
                "payload ".repeat(16).trim_end()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(workspace_dir.join(path), content).expect("write large file");
}

fn seed_single_huge_line_file(workspace_dir: &std::path::Path, path: &str) {
    let content = format!("line 0001: {}", "payload ".repeat(4_000).trim_end());
    std::fs::write(workspace_dir.join(path), content).expect("write single huge line file");
}

#[tokio::test]
async fn read_file_truncates_large_output_and_guides_next_chunk() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-chunking";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);
    let path = "big.txt";
    seed_large_file(&workspace_dir, path);

    let result = server
        .handle_read_file(json!({ "path": path }), Some(session_id.to_string()))
        .await
        .expect("readFile should succeed");

    let text = extract_text_content(&result);
    assert!(
        text.len() < TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES,
        "readFile output must stay inline, got {} bytes",
        text.len()
    );
    assert!(
        text.contains("truncated to stay under the inline limit"),
        "chunk summary should explain why the preview stopped: {text}"
    );
    assert!(
        text.contains("Next chunk: readFile({\"path\": \"big.txt\", \"offset\":"),
        "response should tell the agent how to continue reading: {text}"
    );
    assert!(
        !text.contains("💡 Next:"),
        "readFile success should not append edit-promotion next-action hints: {text}"
    );
    assert!(
        !text.contains("writeFile for full file replacement"),
        "readFile success should not promote writeFile: {text}"
    );
    assert!(
        !text.contains("strReplace.old_string"),
        "readFile success should not expose internal param names: {text}"
    );
    assert!(
        !text.contains("line truncated to fit inline limit"),
        "readFile should only emit complete lines: {text}"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");
    assert_eq!(structured["path"], json!(path));
    assert_eq!(structured["truncated"], json!(true));

    let end_line = structured["endLine"]
        .as_u64()
        .expect("endLine should be present");
    let next_start_line = structured["nextStartLine"]
        .as_u64()
        .expect("nextStartLine should be present");
    let suggested_end_line = structured["suggestedEndLine"]
        .as_u64()
        .expect("suggestedEndLine should be present");

    assert_eq!(next_start_line, end_line + 1);
    assert!(suggested_end_line >= next_start_line);
}

#[tokio::test]
async fn read_file_followup_chunk_uses_guided_line_range() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-next-chunk";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);
    let path = "big.txt";
    seed_large_file(&workspace_dir, path);

    let first_result = server
        .handle_read_file(json!({ "path": path }), Some(session_id.to_string()))
        .await
        .expect("first readFile should succeed");
    let first_structured = first_result
        .structured_content
        .as_ref()
        .expect("structured content expected");

    let next_start_line = first_structured["nextStartLine"]
        .as_u64()
        .expect("nextStartLine should be present");
    let suggested_end_line = first_structured["suggestedEndLine"]
        .as_u64()
        .expect("suggestedEndLine should be present");

    let next_result = server
        .handle_read_file(
            json!({
                "path": path,
                "offset": next_start_line,
                "size": (suggested_end_line - next_start_line + 1)
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("follow-up readFile should succeed");

    let next_text = extract_text_content(&next_result);
    assert!(
        next_text.len() < TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES,
        "follow-up chunk must stay inline, got {} bytes",
        next_text.len()
    );
    assert!(
        next_text.contains(&format!("line {next_start_line:04}:")),
        "follow-up chunk should start where the previous guidance pointed: {next_text}"
    );

    let next_structured = next_result
        .structured_content
        .as_ref()
        .expect("structured content expected");
    assert_eq!(next_structured["startLine"], json!(next_start_line));
}

#[tokio::test]
async fn read_file_empty_file_preserves_standard_success_shape() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-empty";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);
    let path = "empty.txt";
    std::fs::write(workspace_dir.join(path), "").expect("write empty file");

    let result = server
        .handle_read_file(json!({ "path": path }), Some(session_id.to_string()))
        .await
        .expect("empty readFile should succeed");

    let text = extract_text_content(&result);
    assert!(
        text.contains("📄 **`empty.txt`**"),
        "empty-file response should still use the standard readFile success wrapper: {text}"
    );
    assert!(
        text.contains("no lines shown"),
        "empty-file response should explain that the file has no readable lines: {text}"
    );
    assert!(
        !text.contains("Next chunk: readFile("),
        "empty-file response should not suggest a nonexistent follow-up chunk: {text}"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");
    assert_eq!(structured["path"], json!(path));
    assert_eq!(structured["content"], json!(""));
    assert_eq!(structured["size"], json!(0));
    assert_eq!(structured["lines"], json!(0));
    assert_eq!(structured["startLine"], json!(1));
    assert_eq!(structured["endLine"], json!(1));
    assert_eq!(structured["truncated"], json!(false));
    assert_eq!(structured["nextStartLine"], serde_json::Value::Null);
    assert_eq!(structured["suggestedEndLine"], serde_json::Value::Null);
    assert_eq!(structured["nextLineTooLarge"], json!(false));
}

#[cfg(feature = "workspace-edit-file")]
#[tokio::test]
async fn read_file_with_anchors_uses_more_conservative_line_budget() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-anchors";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);
    let path = "big.txt";
    seed_large_file(&workspace_dir, path);

    let result = server
        .handle_read_file(
            json!({ "path": path, "showLineAnchors": true }),
            Some(session_id.to_string()),
        )
        .await
        .expect("anchored readFile should succeed");

    let text = extract_text_content(&result);
    assert!(
        text.len() < TOOL_RESULT_SPILLOVER_THRESHOLD_BYTES,
        "anchored readFile output must stay inline, got {} bytes",
        text.len()
    );
    assert!(
        text.contains("Line format:"),
        "anchored response should keep anchor guidance: {text}"
    );
    assert!(
        text.contains(
            tauri_mcp_agent_lib::mcp::builtin::workspace::edit_mode::read_file_anchor_output_suffix(
            )
        ),
        "anchored response should include mode-specific anchor guidance: {text}"
    );
    assert!(
        !text.contains("line truncated to fit inline limit"),
        "anchored output should still be cut on full lines only: {text}"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");
    assert_eq!(structured["truncated"], json!(true));
    assert_eq!(
        structured["nextStartLine"],
        json!(
            structured["endLine"]
                .as_u64()
                .expect("endLine should be present")
                + 1
        )
    );
}

#[tokio::test]
async fn read_file_guides_single_line_retry_when_next_line_is_too_large() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-single-line-too-large";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);
    let path = "huge-line.txt";
    seed_single_huge_line_file(&workspace_dir, path);

    let result = server
        .handle_read_file(
            json!({ "path": path, "showLineAnchors": true }),
            Some(session_id.to_string()),
        )
        .await
        .expect("single-line readFile should succeed");

    let text = extract_text_content(&result);
    assert!(
        text.contains(
            "Inspect that line directly with readFile({\"path\": \"huge-line.txt\", \"offset\": 1, \"size\": 1})"
        ),
        "response should include an exact single-line retry command: {text}"
    );
    assert!(
        text.contains("Do not rerun readFile on a broader range"),
        "response should warn against repeating a too-broad read: {text}"
    );
    #[cfg(feature = "workspace-edit-file")]
    assert!(
        text.contains("rerun the same 1-line range without showLineAnchors"),
        "anchored retry guidance should mention dropping anchors if needed: {text}"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");
    assert_eq!(structured["truncated"], json!(true));
    assert_eq!(structured["nextLineTooLarge"], json!(true));
    assert_eq!(structured["nextStartLine"], json!(1));
    assert_eq!(structured["suggestedEndLine"], json!(1));
}

#[tokio::test]
async fn read_file_supports_offset_and_size_forward() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-offset-size";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);
    let path = "test.txt";
    std::fs::write(
        workspace_dir.join(path),
        "line 1\nline 2\nline 3\nline 4\nline 5",
    )
    .expect("write file");

    let result = server
        .handle_read_file(
            json!({ "path": path, "offset": 2, "size": 3 }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readFile should succeed");

    let text = extract_text_content(&result);
    assert!(text.contains("line 2"));
    assert!(text.contains("line 3"));
    assert!(text.contains("line 4"));
    assert!(!text.contains("line 1"));
    assert!(!text.contains("line 5"));
}

#[tokio::test]
async fn read_file_supports_size_negative_tail() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-tail";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);
    let path = "test.txt";
    std::fs::write(
        workspace_dir.join(path),
        "line 1\nline 2\nline 3\nline 4\nline 5",
    )
    .expect("write file");

    let result = server
        .handle_read_file(
            json!({ "path": path, "size": -3 }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readFile should succeed");

    let text = extract_text_content(&result);
    assert!(text.contains("line 3"));
    assert!(text.contains("line 4"));
    assert!(text.contains("line 5"));
    assert!(!text.contains("line 1"));
    assert!(!text.contains("line 2"));
}

#[tokio::test]
async fn read_file_supports_offset_and_size_negative_tail_skip() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-file-tail-skip";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);
    let path = "test.txt";
    std::fs::write(
        workspace_dir.join(path),
        "line 1\nline 2\nline 3\nline 4\nline 5",
    )
    .expect("write file");

    let result = server
        .handle_read_file(
            json!({ "path": path, "offset": -2, "size": -2 }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readFile should succeed");

    let text = extract_text_content(&result);
    // skip 2 lines from end (skips line 4 & line 5) and reads 2 lines backwards (line 2 & line 3)
    assert!(text.contains("line 2"));
    assert!(text.contains("line 3"));
    assert!(!text.contains("line 1"));
    assert!(!text.contains("line 4"));
    assert!(!text.contains("line 5"));
}
