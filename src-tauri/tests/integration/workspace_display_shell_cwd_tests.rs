use tauri_mcp_agent_lib::mcp::builtin::workspace::utils::display_shell_cwd;

#[test]
fn display_shell_cwd_normalizes_docker_workspace_root() {
    assert_eq!(
        display_shell_cwd("/workspace/src", "/workspace", true),
        "./src"
    );
    assert_eq!(display_shell_cwd("/workspace", "/workspace", true), ".");
    assert_eq!(
        display_shell_cwd("/home/user/project/src", "/home/user/project", false),
        "./src"
    );
}

#[test]
fn display_shell_cwd_host_exact_match_returns_dot() {
    assert_eq!(
        display_shell_cwd("/home/user/project", "/home/user/project", false),
        "."
    );
}

#[test]
fn display_shell_cwd_unrelated_path_passes_through() {
    assert_eq!(
        display_shell_cwd("/tmp/other", "/home/user/project", false),
        "/tmp/other"
    );
}

#[test]
fn display_shell_cwd_docker_outside_workspace_passes_through() {
    assert_eq!(
        display_shell_cwd("/home/user", "/workspace", true),
        "/home/user"
    );
}
