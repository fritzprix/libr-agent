use tauri_mcp_agent_lib::mcp::builtin::utils::{normalize_user_path, SecurityValidator};
use tempfile::tempdir;

#[test]
fn test_normalize_user_path_relative_unchanged() {
    assert_eq!(
        normalize_user_path("src/mcp/builtin/utils.rs"),
        "src/mcp/builtin/utils.rs"
    );
    assert_eq!(
        normalize_user_path("./subdir/file.txt"),
        "./subdir/file.txt"
    );
}

#[test]
#[cfg(windows)]
fn test_normalize_user_path_windows_formats() {
    assert_eq!(
        normalize_user_path("/C:/Users/example/project"),
        "C:/Users/example/project"
    );
    assert_eq!(
        normalize_user_path("\\C:\\Users\\example\\project"),
        "C:/Users/example/project"
    );
    assert_eq!(
        normalize_user_path("file:///C:/Users/example/project"),
        "C:/Users/example/project"
    );
    assert_eq!(
        normalize_user_path("file://localhost/C:/Users/example/project"),
        "C:/Users/example/project"
    );
    assert_eq!(
        normalize_user_path("/c/Users/example/project"),
        "c:/Users/example/project"
    );
}

#[test]
#[cfg(unix)]
fn test_normalize_user_path_unix_file_url_and_msys_passthrough() {
    assert_eq!(
        normalize_user_path("file:///home/example/project/readme.md"),
        "/home/example/project/readme.md"
    );
    // On Unix, `/c/...` is a real absolute path and must not be rewritten as a drive path.
    assert_eq!(
        normalize_user_path("/c/Users/example/project"),
        "/c/Users/example/project"
    );
}

#[test]
#[cfg(windows)]
fn test_scoped_validator_windows_leading_slash() {
    let temp_dir = tempdir().expect("temp dir");
    let validator = SecurityValidator::new_scoped_with_base_dir(temp_dir.path().to_path_buf());
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "hello").expect("write test file");

    let raw_path = test_file.to_string_lossy();
    let leading_slash_path = format!("/{}", raw_path.replace('\\', "/"));

    let validated = validator
        .validate_path_for_read(&leading_slash_path)
        .expect("absolute path with leading slash within base_dir must succeed");
    assert_eq!(
        validated.canonicalize().unwrap(),
        test_file.canonicalize().unwrap()
    );
}

#[test]
#[cfg(windows)]
fn test_scoped_validator_rejects_outside_base_leading_slash_drive_path() {
    let temp_dir = tempdir().expect("temp dir");
    let validator = SecurityValidator::new_scoped_with_base_dir(temp_dir.path().to_path_buf());

    let outside = normalize_user_path("/C:/Windows/System32/drivers/etc/hosts");
    let result = validator.validate_path_for_read(&outside);
    assert!(
        result.is_err(),
        "path outside base_dir must be rejected, got: {result:?}"
    );
}

#[test]
fn test_scoped_validator_file_url_within_base() {
    let temp_dir = tempdir().expect("temp dir");
    let validator = SecurityValidator::new_scoped_with_base_dir(temp_dir.path().to_path_buf());
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "hello").expect("write test file");

    // Use a non-canonical path in the file:// URL when the OS exposes one (macOS
    // `/var` vs `/private/var`) so we cover the early containment canonicalize path.
    let file_url = url::Url::from_file_path(&test_file)
        .expect("file url")
        .to_string();

    let validated = validator
        .validate_path_for_read(&file_url)
        .expect("file:// URL within base_dir must succeed");
    assert_eq!(
        validated.canonicalize().unwrap(),
        test_file.canonicalize().unwrap()
    );
}

#[cfg(windows)]
mod windows_workspace_glob {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    use tauri_mcp_agent_lib::mcp::builtin::workspace::WorkspaceServer;
    use tauri_mcp_agent_lib::mcp::types::{MCPContent, MCPResult};
    use tauri_mcp_agent_lib::session::SessionManager;

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
    async fn test_glob_files_handles_windows_absolute_path_with_leading_slash() {
        let temp_dir = tempdir().expect("temp dir");
        let session_id = "glob-files-leading-slash";
        let server = build_workspace_server(temp_dir.path(), session_id);
        let workspace_dir = server.get_workspace_dir(session_id);

        std::fs::write(
            workspace_dir.join("message_bubble.rs"),
            "pub struct MessageBubble;",
        )
        .expect("write file");

        let raw_workspace_path = workspace_dir.to_string_lossy();
        let leading_slash_path = format!("/{}", raw_workspace_path.replace('\\', "/"));

        let glob_result = server
            .call_tool(
                "globFiles",
                json!({
                    "path": leading_slash_path,
                    "filePattern": "*message*bubble*",
                }),
                Some(session_id.to_string()),
            )
            .await
            .expect("globFiles dispatch should succeed");

        let glob_text = extract_text_content(&glob_result);
        assert!(
            glob_text.contains("message_bubble.rs"),
            "Expected glob result to contain message_bubble.rs but got: {glob_text}"
        );
    }
}
