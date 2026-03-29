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
async fn file_name_search_uses_snake_case_skip_keys() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "search-file-name-skip-keys";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::create_dir_all(workspace_dir.join("src")).expect("src dir");
    std::fs::create_dir_all(workspace_dir.join("node_modules/pkg")).expect("node_modules dir");
    std::fs::write(workspace_dir.join("src/main.ts"), "const needle = true;\n")
        .expect("write source file");
    std::fs::write(
        workspace_dir.join("node_modules/pkg/index.ts"),
        "const hidden = true;\n",
    )
    .expect("write dependency file");

    let result = server
        .handle_search(
            json!({
                "path": ".",
                "filePattern": "*.ts",
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("search should succeed");

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");

    assert_eq!(structured["skipped_directories"], json!(1));
    assert_eq!(structured["skipped_heavyweight_directories"], json!(1));
    assert_eq!(structured["skipped_gitignored_directories"], json!(0));
    assert!(
        structured.get("skippedDirectories").is_none(),
        "legacy camelCase key should be absent: {structured}"
    );
}

#[tokio::test]
async fn search_skips_heavy_directories_during_recursive_content_search() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "search-skip-heavy-dirs";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::create_dir_all(workspace_dir.join("src")).expect("src dir");
    std::fs::create_dir_all(workspace_dir.join("node_modules/pkg")).expect("node_modules dir");
    std::fs::write(workspace_dir.join("src/main.ts"), "const needle = true;\n")
        .expect("write source file");
    std::fs::write(
        workspace_dir.join("node_modules/pkg/index.js"),
        "const needle = true;\n",
    )
    .expect("write dependency file");

    let result = server
        .handle_search(
            json!({
                "path": ".",
                "query": "needle",
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("search should succeed");

    let text = extract_text_content(&result);
    assert!(
        text.contains("src/main.ts"),
        "expected workspace file in output: {text}"
    );
    assert!(
        !text.contains("node_modules/pkg/index.js"),
        "node_modules should be skipped: {text}"
    );
    assert!(
        text.contains("Skipped 1 heavyweight directory"),
        "skip summary should be visible: {text}"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");
    assert_eq!(structured["files_with_matches"], json!(1));
    assert_eq!(structured["skipped_directories"], json!(1));
    assert_eq!(structured["skipped_heavyweight_directories"], json!(1));
    assert_eq!(structured["skipped_gitignored_directories"], json!(0));
}

#[tokio::test]
async fn search_respects_gitignore_rules_during_recursive_content_search() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "search-gitignore";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::create_dir_all(workspace_dir.join("src")).expect("src dir");
    std::fs::create_dir_all(workspace_dir.join("generated")).expect("generated dir");
    std::fs::write(workspace_dir.join(".gitignore"), "generated/\n").expect("write gitignore");
    std::fs::write(workspace_dir.join("src/main.ts"), "const needle = true;\n")
        .expect("write source file");
    std::fs::write(
        workspace_dir.join("generated/ignored.ts"),
        "const needle = true;\n",
    )
    .expect("write ignored file");

    let result = server
        .handle_search(
            json!({
                "path": ".",
                "query": "needle",
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("search should succeed");

    let text = extract_text_content(&result);
    assert!(
        text.contains("src/main.ts"),
        "expected tracked file in output: {text}"
    );
    assert!(
        !text.contains("generated/ignored.ts"),
        "gitignored file should be excluded: {text}"
    );
    assert!(
        text.contains("Skipped 1 .gitignore-matched directory"),
        "gitignore skip summary should be visible: {text}"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");
    assert_eq!(structured["files_with_matches"], json!(1));
    assert_eq!(structured["skipped_directories"], json!(1));
    assert_eq!(structured["skipped_heavyweight_directories"], json!(0));
    assert_eq!(structured["skipped_gitignored_directories"], json!(1));
}

#[tokio::test]
async fn search_rejects_single_files_that_exceed_content_limit() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "search-large-file-limit";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    let large_file_path = workspace_dir.join("large.log");
    let oversized_content = vec![b'a'; 5 * 1024 * 1024 + 1];
    std::fs::write(&large_file_path, oversized_content).expect("write large file");

    let result = server
        .handle_search(
            json!({
                "path": "large.log",
                "query": "needle",
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("search should return MCPResult");

    let text = extract_text_content(&result);
    assert!(
        text.contains("exceeds the search limit"),
        "expected large file rejection message: {text}"
    );
    assert_eq!(result.is_error, Some(true));
}

#[tokio::test]
async fn write_file_duplicate_resource_guidance_keeps_numbered_steps_clean() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "write-file-duplicate-guidance";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::write(workspace_dir.join("art.html"), "<html>existing</html>\n")
        .expect("seed existing file");

    let result = server
        .handle_write_file(
            json!({
                "path": "art.html",
                "content": "<html>new</html>\n",
                "mode": "create",
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("write should return duplicate-resource guidance");

    let text = extract_text_content(&result);

    assert_eq!(
        result.is_error,
        Some(true),
        "duplicate resource currently uses error semantics"
    );
    assert!(
        text.contains("💡 Next Steps:"),
        "missing next-steps header: {text}"
    );
    assert!(
        text.contains("1. Set \"mode\": \"overwrite\" to replace the existing file."),
        "overwrite guidance should be a clean numbered step: {text}"
    );
    assert!(
        text.contains(
            "2. Set \"mode\": \"append\" to add content to the end of the existing file."
        ),
        "append guidance should be a clean numbered step: {text}"
    );
    assert!(
        text.contains("3. Use readFile(\"art.html\") first if you need the current contents before changing the file."),
        "read guidance should be a clean numbered step: {text}"
    );
    assert!(
        text.contains("4. Use editFile(\"art.html\", [{line, line_hash, new_value}]) for targeted edits instead of rewriting the whole file."),
        "edit guidance should be a clean numbered step: {text}"
    );
    assert!(
        !text.contains("5. "),
        "there should not be a phantom blank numbered item: {text}"
    );
}

#[tokio::test]
async fn search_skips_binary_looking_files_even_without_binary_extension() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "search-binary-skip";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::create_dir_all(workspace_dir.join("src")).expect("src dir");
    std::fs::write(workspace_dir.join("src/main.ts"), "const needle = true;\n")
        .expect("write source file");
    std::fs::write(workspace_dir.join("blob"), b"\0needle\0").expect("write binary-ish file");

    let result = server
        .handle_search(
            json!({
                "path": ".",
                "query": "needle",
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("search should succeed");

    let text = extract_text_content(&result);
    assert!(
        text.contains("src/main.ts"),
        "expected text file in output: {text}"
    );
    assert!(
        !text.contains("`blob`"),
        "binary-looking file should be skipped: {text}"
    );
    assert!(
        text.contains("Skipped 1 binary-looking file"),
        "binary skip summary should be visible: {text}"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content expected");
    assert_eq!(structured["files_with_matches"], json!(1));
    assert_eq!(structured["skipped_binary_files"], json!(1));
}
