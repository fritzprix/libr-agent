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

fn extract_resource_html(result: &MCPResult) -> String {
    result
        .content
        .as_ref()
        .expect("content expected")
        .iter()
        .find_map(|content| match content {
            MCPContent::Resource { resource, .. } => {
                resource["text"].as_str().map(ToString::to_string)
            }
            _ => None,
        })
        .expect("resource html expected")
}

fn extract_text_content(result: &MCPResult) -> String {
    result
        .content
        .as_ref()
        .expect("content expected")
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn list_directory_hides_internal_tmp_and_exports_inside_libragent() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "workspace-internal-listing";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::create_dir_all(workspace_dir.join(".libragent/tmp/process_123")).expect("tmp dir");
    std::fs::create_dir_all(workspace_dir.join(".libragent/exports/files")).expect("exports dir");
    std::fs::create_dir_all(workspace_dir.join(".libragent/tool-results"))
        .expect("tool results dir");
    std::fs::write(
        workspace_dir.join(".libragent/teamwork.json"),
        "{\"kind\":\"teamwork\"}",
    )
    .expect("teamwork manifest");

    let result = server
        .handle_list_directory(
            json!({
                "path": ".libragent"
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("listDirectory should succeed");

    let items = result
        .structured_content
        .as_ref()
        .and_then(|value| value.get("items"))
        .and_then(|value| value.as_array())
        .expect("items array expected");

    let names = items
        .iter()
        .filter_map(|item| item.get("name").and_then(|value| value.as_str()))
        .collect::<Vec<_>>();

    assert!(names.contains(&"tool-results"));
    assert!(names.contains(&"teamwork.json"));
    assert!(!names.contains(&"tmp"));
    assert!(!names.contains(&"exports"));
}

#[cfg(unix)]
#[tokio::test]
async fn list_directory_hides_internal_artifacts_when_workspace_root_is_symlinked() {
    use std::os::unix::fs::symlink;

    let temp_dir = tempdir().expect("temp dir");
    let real_base_dir = temp_dir.path().join("real-base");
    std::fs::create_dir_all(&real_base_dir).expect("real base dir");

    let symlink_base_dir = temp_dir.path().join("symlink-base");
    symlink(&real_base_dir, &symlink_base_dir).expect("base dir symlink");

    let session_id = "workspace-internal-listing-symlinked";
    let server = build_workspace_server(&symlink_base_dir, session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::create_dir_all(workspace_dir.join(".libragent/tmp/process_123")).expect("tmp dir");
    std::fs::create_dir_all(workspace_dir.join(".libragent/exports/files")).expect("exports dir");
    std::fs::write(
        workspace_dir.join(".libragent/tmp/process_123/stdout"),
        "process output",
    )
    .expect("tmp artifact");

    let result = server
        .handle_list_directory(
            json!({
                "path": ".libragent"
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("listDirectory should succeed");

    let items = result
        .structured_content
        .as_ref()
        .and_then(|value| value.get("items"))
        .and_then(|value| value.as_array())
        .expect("items array expected");

    let names = items
        .iter()
        .filter_map(|item| item.get("name").and_then(|value| value.as_str()))
        .collect::<Vec<_>>();

    assert!(!names.contains(&"tmp"));
    assert!(!names.contains(&"exports"));
}

#[tokio::test]
async fn export_response_uses_libragent_download_path() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "workspace-export-download-path";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::create_dir_all(workspace_dir.join("src")).expect("src dir");
    std::fs::write(workspace_dir.join("src/report.txt"), "hello export").expect("source file");

    let result = server
        .handle_export(
            json!({
                "paths": ["src/report.txt"]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("export should succeed");

    let html = extract_resource_html(&result);
    let text = extract_text_content(&result);
    assert!(
        html.contains(".libragent/exports/files/"),
        "download path should point at .libragent exports: {html}"
    );
    assert!(
        text.contains("Saved export: `.libragent/exports/files/"),
        "text response should expose the saved export path: {text}"
    );
    assert!(
        text.contains("UI resource: `ui://export/"),
        "text response should expose the UI resource URI: {text}"
    );

    let exports_dir = workspace_dir.join(".libragent/exports/files");
    let exported_files = std::fs::read_dir(exports_dir)
        .expect("exports dir")
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    assert_eq!(
        exported_files.len(),
        1,
        "exactly one exported file expected"
    );
}

#[tokio::test]
async fn export_omits_internal_tmp_and_exports_when_packaging_workspace_root() {
    let temp_dir = tempdir().expect("temp dir");
    let session_id = "workspace-export-omits-internal-artifacts";
    let server = build_workspace_server(temp_dir.path(), session_id);
    let workspace_dir = server.get_workspace_dir(session_id);

    std::fs::create_dir_all(workspace_dir.join("src")).expect("src dir");
    std::fs::write(workspace_dir.join("src/keep.txt"), "keep me").expect("workspace file");
    std::fs::create_dir_all(workspace_dir.join(".libragent/tmp/process_123")).expect("tmp dir");
    std::fs::write(
        workspace_dir.join(".libragent/tmp/process_123/stdout"),
        "secret temp output",
    )
    .expect("temp artifact");
    std::fs::create_dir_all(workspace_dir.join(".libragent/exports/files")).expect("exports dir");
    std::fs::write(
        workspace_dir.join(".libragent/exports/files/already-exported.txt"),
        "old export",
    )
    .expect("old exported artifact");

    server
        .handle_export(
            json!({
                "paths": ["."]
            }),
            Some(session_id.to_string()),
        )
        .await
        .expect("zip export should succeed");

    let packages_dir = workspace_dir.join(".libragent/exports/packages");
    let archive_path = std::fs::read_dir(&packages_dir)
        .expect("packages dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("zip"))
        .expect("zip archive expected");

    let archive_file = std::fs::File::open(&archive_path).expect("open archive");
    let mut archive = zip::ZipArchive::new(archive_file).expect("read archive");
    let mut archived_names = Vec::new();
    for index in 0..archive.len() {
        archived_names.push(
            archive
                .by_index(index)
                .expect("archive entry")
                .name()
                .to_string(),
        );
    }

    assert!(
        archived_names.iter().any(|name| name == "src/keep.txt"),
        "expected workspace file in archive: {:?}",
        archived_names
    );
    assert!(
        archived_names
            .iter()
            .all(|name| !name.starts_with(".libragent/tmp/")),
        "tmp artifacts must not leak into exports: {:?}",
        archived_names
    );
    assert!(
        archived_names
            .iter()
            .all(|name| !name.starts_with(".libragent/exports/")),
        "export artifacts must not recursively package themselves: {:?}",
        archived_names
    );
}
