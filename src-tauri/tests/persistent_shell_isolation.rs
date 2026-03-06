use tauri_mcp_agent_lib::mcp::builtin::workspace::persistent_shell::PersistentShell;
use tauri_mcp_agent_lib::session_isolation::types::ShellType;
use std::env;

#[tokio::test]
async fn test_persistent_shell_environment_isolation() {
    // 1. Set a "secret" environment variable in the parent process
    let secret_key = "LIBRAGENT_TEST_SECRET_KEY";
    let secret_value = "super-secret-value-123";
    env::set_var(secret_key, secret_value);

    // 2. Create a temporary workspace
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let workspace_path = temp_dir.path().to_path_buf();

    // 3. Determine shell type based on platform
    let shell_type = if cfg!(windows) {
        ShellType::PowerShell
    } else {
        ShellType::Bash
    };

    // 4. Start a persistent shell
    let mut shell = PersistentShell::new(
        "test-isolation-session".to_string(),
        workspace_path,
        shell_type,
    )
    .await
    .expect("Failed to create persistent shell");

    // 5. Execute a command to print the secret variable
    // We use different commands for different shells
    let cmd = if cfg!(windows) {
        format!("echo $env:{}", secret_key)
    } else {
        format!("echo ${}", secret_key)
    };

    let (stdout, _stderr, _exit_code, _cwd) = shell.execute(&cmd).await.expect("Failed to execute command");

    // 6. Assert the secret is NOT found in the output
    assert!(!stdout.contains(secret_value), "Secret environment variable leaked into persistent shell! Output: {}", stdout);
    
    // 7. Verify whitelisted variables ARE present
    // For example, PATH (or Path on Windows) should exist
    let path_cmd = if cfg!(windows) {
        "echo $env:Path"
    } else {
        "echo $PATH"
    };
    
    let (path_output, _, _, _) = shell.execute(path_cmd).await.expect("Failed to execute path command");
    assert!(!path_output.trim().is_empty(), "PATH environment variable missing in persistent shell!");
}
