use tauri_mcp_agent_lib::commands::workspace_commands::resolve_workspace_scoped_file_path;
use tempfile::tempdir;

#[tokio::test]
async fn allows_local_file_reads_inside_workspace() {
    let workspace = tempdir().expect("workspace temp dir");
    let nested_dir = workspace.path().join("images");
    std::fs::create_dir_all(&nested_dir).expect("create nested dir");
    let file_path = nested_dir.join("tool-output.png");
    std::fs::write(&file_path, b"png").expect("write test file");

    let resolved = resolve_workspace_scoped_file_path(&file_path, workspace.path())
        .await
        .expect("workspace file should be allowed");

    assert_eq!(
        resolved,
        std::fs::canonicalize(&file_path).expect("canonical file path")
    );
}

#[tokio::test]
async fn rejects_local_file_reads_outside_workspace() {
    let workspace = tempdir().expect("workspace temp dir");
    let outside = tempdir().expect("outside temp dir");
    let file_path = outside.path().join("secret.txt");
    std::fs::write(&file_path, b"nope").expect("write outside file");

    let error = resolve_workspace_scoped_file_path(&file_path, workspace.path())
        .await
        .expect_err("outside file should be rejected");

    assert!(error.contains("outside the session workspace"));
}
