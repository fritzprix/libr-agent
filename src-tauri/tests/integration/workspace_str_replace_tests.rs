#![cfg(feature = "workspace-str-replace")]

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
async fn str_replace_replaces_single_unique_match() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "str-replace-single";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let workspace_dir = server.get_workspace_dir(session_id);
    let file_path = workspace_dir.join("demo.txt");
    std::fs::write(&file_path, "alpha\nbeta\ngamma\n").expect("seed file");

    let result = server
        .call_tool(
            "strReplace",
            json!({
                "path": "demo.txt",
                "old_string": "beta",
                "new_string": "BETA"
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("strReplace should return");

    assert!(
        !result.is_error.unwrap_or(true),
        "expected success: {result:?}"
    );
    let text = extract_text_content(&result);
    assert!(text.contains("Replaced 1 occurrence"), "{text}");
    assert!(
        text.contains("@@"),
        "success body should include a unified diff so re-read is unnecessary: {text}"
    );
    assert!(
        !text.contains("readFile to verify"),
        "strReplace must not suggest re-reading after returning a diff: {text}"
    );
    assert!(
        !text.contains("💡 Suggested Follow-ups:"),
        "strReplace success should not append follow-ups: {text}"
    );

    let updated = std::fs::read_to_string(file_path).expect("read updated file");
    assert_eq!(updated, "alpha\nBETA\ngamma\n");
}

#[tokio::test]
async fn str_replace_rejects_ambiguous_match_without_replace_all() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "str-replace-ambiguous";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let workspace_dir = server.get_workspace_dir(session_id);
    let file_path = workspace_dir.join("dup.txt");
    std::fs::write(&file_path, "foo foo foo\n").expect("seed file");

    let result = server
        .call_tool(
            "strReplace",
            json!({
                "path": "dup.txt",
                "old_string": "foo",
                "new_string": "bar"
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("strReplace should return");

    assert!(
        result.is_error.unwrap_or(false),
        "expected error: {result:?}"
    );
    let text = extract_text_content(&result);
    assert!(text.contains("matched 3 times"), "{text}");
    assert_eq!(std::fs::read_to_string(file_path).unwrap(), "foo foo foo\n");
}

#[tokio::test]
async fn str_replace_replace_all_updates_every_match() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "str-replace-all";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let workspace_dir = server.get_workspace_dir(session_id);
    let file_path = workspace_dir.join("all.txt");
    std::fs::write(&file_path, "foo foo foo\n").expect("seed file");

    let result = server
        .call_tool(
            "strReplace",
            json!({
                "path": "all.txt",
                "old_string": "foo",
                "new_string": "bar",
                "replace_all": true
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("strReplace should return");

    assert!(
        !result.is_error.unwrap_or(true),
        "expected success: {result:?}"
    );
    assert_eq!(std::fs::read_to_string(file_path).unwrap(), "bar bar bar\n");
}

#[tokio::test]
async fn str_replace_rejects_missing_old_string() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "str-replace-missing";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let workspace_dir = server.get_workspace_dir(session_id);
    let file_path = workspace_dir.join("missing.txt");
    std::fs::write(&file_path, "hello world\n").expect("seed file");

    let result = server
        .call_tool(
            "strReplace",
            json!({
                "path": "missing.txt",
                "old_string": "goodbye",
                "new_string": "hi"
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("strReplace should return");

    assert!(
        result.is_error.unwrap_or(false),
        "expected error: {result:?}"
    );
    let text = extract_text_content(&result);
    assert!(text.contains("old_string was not found"), "{text}");
}

#[tokio::test]
async fn workspace_file_tools_expose_str_replace_not_edit_file() {
    use tauri_mcp_agent_lib::mcp::builtin::workspace::tools::file_tools;

    let names: Vec<String> = file_tools().into_iter().map(|tool| tool.name).collect();

    assert!(names.contains(&"strReplace".to_string()), "{names:?}");
    assert!(!names.contains(&"editFile".to_string()), "{names:?}");
}

#[tokio::test]
async fn read_file_success_omits_edit_promotion_hints_on_str_replace_builds() {
    use tauri_mcp_agent_lib::mcp::builtin::workspace::tools::file_tools::create_read_file_tool;

    let temp_dir = tempdir().expect("temp dir");
    let session_id = "read-hint-str-replace";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);
    std::fs::write(workspace_dir.join("hint.txt"), "hello\n").expect("seed");

    let result = server
        .call_tool(
            "readFile",
            json!({ "path": "hint.txt" }),
            Some(session_id.to_string()),
        )
        .await
        .expect("readFile should return");

    let text = extract_text_content(&result);
    assert!(
        !text.contains("💡 Next:"),
        "readFile success should not append next-action hints: {text}"
    );
    assert!(
        !text.contains("editFile"),
        "readFile success should not mention editFile on strReplace builds: {text}"
    );

    // Edit affordance remains in the tool schema, not success next-actions.
    let description = create_read_file_tool().description;
    assert!(
        description.contains("strReplace"),
        "readFile schema should still mention strReplace: {description}"
    );
    assert!(
        !description.contains("editFile"),
        "readFile schema should not mention editFile on strReplace builds: {description}"
    );
}

#[tokio::test]
async fn read_file_and_search_schemas_omit_show_line_anchors() {
    use tauri_mcp_agent_lib::mcp::builtin::workspace::tools::file_tools::{
        create_glob_files_tool, create_grep_files_tool, create_read_file_tool, create_search_tool,
    };

    let read_schema = serde_json::to_value(create_read_file_tool().input_schema)
        .expect("serialize readFile schema");
    let grep_schema = serde_json::to_value(create_grep_files_tool().input_schema)
        .expect("serialize grepFiles schema");
    let glob_schema = serde_json::to_value(create_glob_files_tool().input_schema)
        .expect("serialize globFiles schema");
    let search_schema = serde_json::to_value(create_search_tool().input_schema)
        .expect("serialize searchFiles schema");
    let read_props = read_schema["properties"]
        .as_object()
        .expect("readFile properties");
    let grep_props = grep_schema["properties"]
        .as_object()
        .expect("grepFiles properties");
    let glob_props = glob_schema["properties"]
        .as_object()
        .expect("globFiles properties");
    let search_props = search_schema["properties"]
        .as_object()
        .expect("searchFiles properties");

    assert!(
        !read_props.contains_key("showLineAnchors"),
        "readFile schema should not expose showLineAnchors on strReplace builds"
    );
    assert!(
        !grep_props.contains_key("showLineAnchors"),
        "grepFiles schema should not expose showLineAnchors on strReplace builds"
    );
    assert!(
        !glob_props.contains_key("query"),
        "globFiles schema should not expose query"
    );
    assert!(
        !search_props.contains_key("showLineAnchors"),
        "searchFiles schema should not expose showLineAnchors on strReplace builds"
    );

    let glob_required = glob_schema["required"]
        .as_array()
        .expect("globFiles required");
    assert!(glob_required.contains(&json!("path")));
    assert!(glob_required.contains(&json!("filePattern")));

    let grep_required = grep_schema["required"]
        .as_array()
        .expect("grepFiles required");
    assert!(grep_required.contains(&json!("path")));
    assert!(grep_required.contains(&json!("query")));
}

#[tokio::test]
async fn write_file_append_preview_uses_raw_content_without_anchors() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "write-append-raw-preview";
    let server = build_workspace_server(temp_dir.path(), session_id);

    server
        .call_tool(
            "writeFile",
            json!({
                "path": "notes.txt",
                "content": "alpha\n",
                "mode": "create",
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("create should succeed");

    let result = server
        .call_tool(
            "writeFile",
            json!({
                "path": "notes.txt",
                "content": "beta\n",
                "mode": "append",
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("append should succeed");

    let text = extract_text_content(&result);
    assert!(
        text.contains("alpha"),
        "preview should show file content: {text}"
    );
    assert!(
        text.contains("beta"),
        "preview should show appended line: {text}"
    );
    assert!(
        !text.contains("Current anchors"),
        "strReplace builds must not mention anchors in writeFile preview: {text}"
    );
    assert!(
        !text.contains('|'),
        "preview should be raw lines without anchor pipe separators: {text}"
    );
}
