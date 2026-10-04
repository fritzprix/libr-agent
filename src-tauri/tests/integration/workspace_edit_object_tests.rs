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
async fn edit_object_sets_nested_json_field() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-object-set";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let workspace_dir = server.get_workspace_dir(session_id);
    let file_path = workspace_dir.join("package.json");
    std::fs::write(
        &file_path,
        "{\n  \"scripts\": {\n    \"dev\": \"old\"\n  }\n}\n",
    )
    .expect("seed file");

    let result = server
        .call_tool(
            "editObject",
            json!({
                "path": "package.json",
                "ops": [
                    {"op": "set", "path": "/scripts/dev", "value": "vite"},
                    {"op": "set", "path": "/scripts/lint", "value": "eslint"}
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("editObject should return");

    assert!(
        !result.is_error.unwrap_or(true),
        "expected success: {}",
        extract_text_content(&result)
    );
    let text = extract_text_content(&result);
    assert!(text.contains("applied 2 op"), "{text}");
    assert!(
        result.structured_content.is_some(),
        "structured_content expected"
    );

    let updated: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
    assert_eq!(updated["scripts"]["dev"], "vite");
    assert_eq!(updated["scripts"]["lint"], "eslint");
    let raw = std::fs::read_to_string(&file_path).unwrap();
    assert!(raw.ends_with('\n'));
    assert!(raw.contains("\n  "));
}

#[tokio::test]
async fn edit_object_root_set_replaces_document() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-object-root";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let workspace_dir = server.get_workspace_dir(session_id);
    let file_path = workspace_dir.join("config.json");
    std::fs::write(&file_path, "{\"a\":1}\n").expect("seed file");

    let result = server
        .call_tool(
            "editObject",
            json!({
                "path": "config.json",
                "ops": [
                    {"op": "set", "path": "/", "value": {"b": 2}}
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("editObject should return");

    assert!(!result.is_error.unwrap_or(true), "{:?}", result);
    let updated: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(file_path).unwrap()).unwrap();
    assert_eq!(updated, json!({"b": 2}));
}

#[tokio::test]
async fn edit_object_append_and_remove_array() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-object-array";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let workspace_dir = server.get_workspace_dir(session_id);
    let file_path = workspace_dir.join("data.json");
    std::fs::write(&file_path, "{\"keywords\":[\"a\",\"b\"]}\n").expect("seed file");

    let result = server
        .call_tool(
            "editObject",
            json!({
                "path": "data.json",
                "ops": [
                    {"op": "append", "path": "/keywords", "value": "c"},
                    {"op": "remove", "path": "/keywords/0"}
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("editObject should return");

    assert!(
        !result.is_error.unwrap_or(true),
        "{}",
        extract_text_content(&result)
    );
    let updated: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(file_path).unwrap()).unwrap();
    assert_eq!(updated["keywords"], json!(["b", "c"]));
}

#[tokio::test]
async fn edit_object_is_atomic_on_mid_batch_failure() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-object-atomic";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let workspace_dir = server.get_workspace_dir(session_id);
    let file_path = workspace_dir.join("data.json");
    let original = "{\n  \"keywords\": [\n    \"a\"\n  ]\n}\n";
    std::fs::write(&file_path, original).expect("seed file");

    let result = server
        .call_tool(
            "editObject",
            json!({
                "path": "data.json",
                "ops": [
                    {"op": "append", "path": "/keywords", "value": "b"},
                    {"op": "append", "path": "/missing", "value": "x"}
                ]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("editObject should return");

    assert!(result.is_error.unwrap_or(false), "expected error");
    let text = extract_text_content(&result);
    assert!(text.contains("ops[1]"), "{text}");
    assert_eq!(std::fs::read_to_string(file_path).unwrap(), original);
}

#[tokio::test]
async fn edit_object_rejects_non_json_extension() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-object-ext";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let workspace_dir = server.get_workspace_dir(session_id);
    std::fs::write(workspace_dir.join("demo.yaml"), "a: 1\n").expect("seed file");

    let result = server
        .call_tool(
            "editObject",
            json!({
                "path": "demo.yaml",
                "ops": [{"op": "set", "path": "/a", "value": 2}]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("editObject should return");

    assert!(result.is_error.unwrap_or(false));
    let text = extract_text_content(&result);
    assert!(text.contains(".json"), "{text}");
}

#[tokio::test]
async fn edit_object_rejects_invalid_json_on_disk() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-object-invalid";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let workspace_dir = server.get_workspace_dir(session_id);
    let file_path = workspace_dir.join("broken.json");
    std::fs::write(&file_path, "{\n  // comment\n  \"a\": 1\n}\n").expect("seed file");

    let result = server
        .call_tool(
            "editObject",
            json!({
                "path": "broken.json",
                "ops": [{"op": "set", "path": "/a", "value": 2}]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("editObject should return");

    assert!(result.is_error.unwrap_or(false));
    let text = extract_text_content(&result);
    assert!(text.contains("Invalid JSON"), "{text}");
    assert!(
        std::fs::read_to_string(file_path)
            .unwrap()
            .contains("// comment"),
        "file must remain unchanged"
    );
}

#[tokio::test]
async fn edit_object_is_registered_in_file_tools() {
    let tools = tauri_mcp_agent_lib::mcp::builtin::workspace::tools::file_tools();
    assert!(
        tools.iter().any(|tool| tool.name == "editObject"),
        "editObject must always be registered"
    );
}

#[tokio::test]
async fn edit_object_removes_nested_array_row_by_pointer() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-object-nested-remove";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let workspace_dir = server.get_workspace_dir(session_id);
    let file_path = workspace_dir.join("grid.json");
    std::fs::write(&file_path, "{\"grid\":[[1,2],[3,4]]}\n").expect("seed file");

    let result = server
        .call_tool(
            "editObject",
            json!({
                "path": "grid.json",
                "ops": [{"op": "remove", "path": "/grid/0"}]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("editObject should return");

    assert!(
        !result.is_error.unwrap_or(true),
        "{}",
        extract_text_content(&result)
    );
    let updated: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(file_path).unwrap()).unwrap();
    assert_eq!(updated["grid"], json!([[3, 4]]));
}

#[tokio::test]
async fn edit_object_rejects_legacy_index_field() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-object-legacy-index";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let workspace_dir = server.get_workspace_dir(session_id);
    std::fs::write(workspace_dir.join("data.json"), "{\"keywords\":[\"a\"]}\n").expect("seed file");

    let result = server
        .call_tool(
            "editObject",
            json!({
                "path": "data.json",
                "ops": [{"op": "remove", "path": "/keywords", "index": 0}]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("editObject should return");

    assert!(result.is_error.unwrap_or(false));
    let text = extract_text_content(&result);
    assert!(text.contains("index"), "{text}");
}

#[tokio::test]
async fn edit_object_summarizes_long_multibyte_values_without_panic() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "edit-object-utf8";
    let server = build_workspace_server(temp_dir.path(), session_id);

    let workspace_dir = server.get_workspace_dir(session_id);
    let file_path = workspace_dir.join("i18n.json");
    std::fs::write(&file_path, "{\"msg\":\"old\"}\n").expect("seed file");

    let long_value = "가".repeat(80);
    let result = server
        .call_tool(
            "editObject",
            json!({
                "path": "i18n.json",
                "ops": [{"op": "set", "path": "/msg", "value": long_value}]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("editObject should return");

    assert!(
        !result.is_error.unwrap_or(true),
        "{}",
        extract_text_content(&result)
    );
    let updated: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(file_path).unwrap()).unwrap();
    assert_eq!(updated["msg"].as_str().unwrap().chars().count(), 80);
}
