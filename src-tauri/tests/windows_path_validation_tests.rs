use std::path::Path;
use tauri_mcp_agent_lib::mcp::builtin::utils::{
    display_workspace_path, normalize_user_path, strip_windows_verbatim_prefix, SecurityValidator,
};
use tempfile::tempdir;

#[test]
fn strip_windows_verbatim_prefix_removes_drive_forms() {
    assert_eq!(
        strip_windows_verbatim_prefix(r"\\?\C:\Users\me\file.html"),
        r"C:\Users\me\file.html"
    );
    assert_eq!(
        strip_windows_verbatim_prefix("//?/C:/Users/me/file.html"),
        "C:/Users/me/file.html"
    );
    assert_eq!(
        strip_windows_verbatim_prefix(r"C:\Users\me\file.html"),
        r"C:\Users\me\file.html"
    );
}

#[test]
fn strip_windows_verbatim_prefix_preserves_unc() {
    assert_eq!(
        strip_windows_verbatim_prefix(r"\\?\UNC\server\share\file.html"),
        r"\\?\UNC\server\share\file.html"
    );
    assert_eq!(
        strip_windows_verbatim_prefix("//?/UNC/server/share/file.html"),
        "//?/UNC/server/share/file.html"
    );
}

#[test]
fn strip_windows_verbatim_prefix_edge_cases() {
    assert_eq!(strip_windows_verbatim_prefix(""), "");
    assert_eq!(strip_windows_verbatim_prefix("//?"), "//?");
    assert_eq!(
        strip_windows_verbatim_prefix(r"//?/UNC\server\share\file.html"),
        r"//?/UNC\server\share\file.html"
    );
    assert_eq!(
        strip_windows_verbatim_prefix(r"\\?\UNC/server/share/file.html"),
        r"\\?\UNC/server/share/file.html"
    );
}

#[test]
fn display_workspace_path_strips_verbatim_drive_prefix() {
    let path = Path::new(r"\\?\C:\Users\me\workspace");
    assert_eq!(display_workspace_path(path), r"C:\Users\me\workspace");
}

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
    // Bare `~name` (no slash) is not a home shortcut.
    assert_eq!(normalize_user_path("~backup"), "~backup");
}

#[test]
fn test_normalize_user_path_expands_home_shortcuts() {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let home = home.to_string_lossy().replace('\\', "/");

    assert_eq!(normalize_user_path("~"), home);
    assert_eq!(
        normalize_user_path("~/my_works/project/docs/PLANNING.md"),
        format!("{home}/my_works/project/docs/PLANNING.md")
    );
    assert_eq!(
        normalize_user_path("$HOME/my_works/project"),
        format!("{home}/my_works/project")
    );
    assert_eq!(
        normalize_user_path("${HOME}/my_works/project"),
        format!("{home}/my_works/project")
    );
}

#[test]
fn test_scoped_validator_rejects_tilde_outside_workspace() {
    let temp_dir = tempdir().expect("temp dir");
    let validator = SecurityValidator::new_scoped_with_base_dir(temp_dir.path().to_path_buf());
    let Some(home) = dirs::home_dir() else {
        return;
    };
    // Ensure the expanded path is outside this temp workspace.
    if home.starts_with(temp_dir.path()) {
        return;
    }

    let result = validator.validate_path_for_write("~/outside_workspace_file.txt");
    assert!(
        result.is_err(),
        "tilde path outside workspace must be rejected (not create literal '~' dir); got {result:?}"
    );

    // Relative join under workspace must never produce workspace/~/...
    let literal_tilde = temp_dir.path().join("~").join("outside_workspace_file.txt");
    assert!(
        !literal_tilde.exists(),
        "must not create a literal '~' directory under the workspace"
    );
}

#[test]
fn test_scoped_validator_allows_tilde_when_workspace_is_home() {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let validator = SecurityValidator::new_scoped_with_base_dir(home.clone());
    let expected = home.join("libragent_tilde_path_test.txt");
    let resolved = validator
        .validate_path_for_write("~/libragent_tilde_path_test.txt")
        .expect("tilde under home workspace should resolve");
    assert_eq!(resolved, expected);
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

mod attachment_workspace_path_exposure {
    use std::fs;
    use std::path::Path;
    use tauri_mcp_agent_lib::mcp::builtin::attachments::helpers::{
        exposed_src_url, exposed_workspace_path_from_src_url, path_is_within_workspace,
        persistable_attachment_src_url,
    };
    use tempfile::tempdir;

    fn path_to_file_url(path: &Path) -> String {
        url::Url::from_file_path(path)
            .expect("absolute path")
            .to_string()
    }

    #[test]
    fn exposes_relative_workspace_path_and_never_host_file_urls() {
        let workspace = tempdir().expect("workspace tempdir");
        let nested = workspace.path().join("attachments");
        fs::create_dir_all(&nested).expect("mkdir attachments");
        let inside = nested.join("note.txt");
        fs::write(&inside, b"hi").expect("write inside");

        let outside_root = tempdir().expect("outside tempdir");
        let outside = outside_root.path().join("secret.txt");
        fs::write(&outside, b"nope").expect("write outside");

        let inside_url = path_to_file_url(&inside);
        let outside_url = path_to_file_url(&outside);

        assert_eq!(
            exposed_workspace_path_from_src_url(&inside_url, workspace.path()).as_deref(),
            Some("attachments/note.txt")
        );
        // Agent-facing srcUrl must not echo local file:// paths.
        assert_eq!(exposed_src_url(Some(&inside_url)), None);
        assert_eq!(exposed_src_url(Some(&outside_url)), None);
        assert_eq!(
            exposed_src_url(Some("https://example.com/doc.pdf")).as_deref(),
            Some("https://example.com/doc.pdf")
        );

        assert_eq!(
            exposed_workspace_path_from_src_url(&outside_url, workspace.path()),
            None
        );

        assert_eq!(
            persistable_attachment_src_url(None, Some(&outside_url), workspace.path()),
            None
        );
        assert_eq!(
            persistable_attachment_src_url(None, Some(&inside_url), workspace.path()).as_deref(),
            Some(inside_url.as_str())
        );
        assert_eq!(
            persistable_attachment_src_url(
                Some("https://example.com/a"),
                Some(&outside_url),
                workspace.path()
            )
            .as_deref(),
            Some("https://example.com/a")
        );
        // In-workspace file:// wins over remote so workspacePath can be derived.
        assert_eq!(
            persistable_attachment_src_url(
                Some("https://example.com/a"),
                Some(&inside_url),
                workspace.path()
            )
            .as_deref(),
            Some(inside_url.as_str())
        );
    }

    #[test]
    fn path_is_within_workspace_fail_closed_when_missing() {
        let workspace = tempdir().expect("workspace");
        let missing = workspace.path().join("does-not-exist.txt");
        assert!(!path_is_within_workspace(&missing, workspace.path()));
    }

    #[test]
    fn exposed_and_persistable_reject_raw_paths_and_non_http_schemes() {
        let workspace = tempdir().expect("workspace");
        let inside = workspace.path().join("ok.txt");
        fs::write(&inside, b"ok").expect("write");
        let inside_url = path_to_file_url(&inside);

        assert_eq!(exposed_src_url(Some(r"C:\Windows\System32\cmd.exe")), None);
        assert_eq!(exposed_src_url(Some("/etc/passwd")), None);
        assert_eq!(exposed_src_url(Some("blob:https://example.com/1")), None);
        assert_eq!(exposed_src_url(Some("data:text/plain,hi")), None);

        // Case-insensitive file scheme still derives workspacePath, never echoed as srcUrl.
        let upper = inside_url.replacen("file://", "FILE://", 1);
        assert_eq!(
            exposed_workspace_path_from_src_url(&upper, workspace.path()).as_deref(),
            Some("ok.txt")
        );
        assert_eq!(exposed_src_url(Some(&upper)), None);

        assert_eq!(
            persistable_attachment_src_url(
                Some(r"C:\secret.txt"),
                Some("blob:https://example.com/x"),
                workspace.path()
            ),
            None
        );
        assert_eq!(
            persistable_attachment_src_url(Some(&upper), None, workspace.path()).as_deref(),
            Some(upper.as_str())
        );
    }
}
