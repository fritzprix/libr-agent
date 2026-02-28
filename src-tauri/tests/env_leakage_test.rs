#[cfg(unix)]
#[tokio::test]
async fn test_env_leakage_in_unix_isolation() {
    use std::collections::HashMap;
    use tauri_mcp_agent_lib::session_isolation::platforms::create_basic_isolated_command;
    use tauri_mcp_agent_lib::session_isolation::types::{
        IsolatedProcessConfig, IsolationLevel, ShellType,
    };

    // Set a secret in the parent process
    std::env::set_var("SECRET_API_KEY", "super_secret_value");

    let config = IsolatedProcessConfig {
        session_id: "test-session".to_string(),
        workspace_path: std::env::temp_dir(),
        command: "env".to_string(), // Execute 'env' to list variables
        args: vec![],
        env_vars: HashMap::new(),
        isolation_level: IsolationLevel::Basic,
        shell_type: Some(ShellType::Bash),
    };

    let mut cmd: tokio::process::Command = create_basic_isolated_command(config)
        .await
        .expect("Failed to create command");

    let output = cmd.output().await.expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify that the secret is NOT present in the child environment
    assert!(
        !stdout.contains("SECRET_API_KEY=super_secret_value"),
        "Environment variable leaked! Secret found in output."
    );

    // Verify that essential environment variables are preserved
    assert!(
        stdout.contains("PATH="),
        "Expected PATH to be present in isolated environment, but it was missing."
    );

    // If TERM is set in the parent, it should also be present in the isolated environment
    if std::env::var("TERM").is_ok() {
        assert!(
            stdout.contains("TERM="),
            "TERM is set in the parent environment but missing in the isolated environment."
        );
    }

    // Clean up to prevent affecting other tests in the same process
    std::env::remove_var("SECRET_API_KEY");
}
