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
        text.contains("Next chunk: readFile({\"path\": \"big.txt\", \"startLine\":"),
        "response should tell the agent how to continue reading: {text}"
    );
    assert!(
        text.contains(
            "If you plan to use editFiles next, rerun with showLineAnchors=true to get anchors"
        ),
        "plain readFile should keep anchor guidance optional instead of sounding mandatory: {text}"
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
                "startLine": next_start_line,
                "endLine": suggested_end_line
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
        text.contains("Do not pass `1:792c6f`"),
        "anchored response should clarify that only the 6-character anchor is valid: {text}"
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
            "Inspect that line directly with readFile({\"path\": \"huge-line.txt\", \"startLine\": 1, \"endLine\": 1})"
        ),
        "response should include an exact single-line retry command: {text}"
    );
    assert!(
        text.contains("Do not rerun readFile on a broader range"),
        "response should warn against repeating a too-broad read: {text}"
    );
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
