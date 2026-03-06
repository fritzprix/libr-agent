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

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_env_leakage_in_linux_high_isolation() {
    use std::collections::HashMap;
    use tauri_mcp_agent_lib::session_isolation::platforms::linux::create_high_isolated_command;
    use tauri_mcp_agent_lib::session_isolation::types::{
        IsolatedProcessConfig, IsolationConfig, IsolationLevel, ResourceLimits,
    };

    // Skip if unshare is not available in this environment
    let unshare_available = std::process::Command::new("unshare")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !unshare_available {
        eprintln!("Skipping test_env_leakage_in_linux_high_isolation: unshare not available");
        return;
    }

    // Set secrets in the parent process
    std::env::set_var("SECRET_API_KEY", "super_secret_value");
    std::env::set_var("XDG_RUNTIME_DIR", "/run/user/9999");

    let isolation_config = IsolationConfig {
        resource_limits: ResourceLimits::default(),
    };

    let config = IsolatedProcessConfig {
        session_id: "test-high-session".to_string(),
        workspace_path: std::env::temp_dir(),
        command: "env".to_string(),
        args: vec![],
        env_vars: HashMap::new(),
        isolation_level: IsolationLevel::High,
        shell_type: None,
    };

    let mut cmd = create_high_isolated_command(config, &isolation_config)
        .await
        .expect("Failed to create high isolation command");

    let output = cmd.output().await.expect("Failed to run command");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Host secret must not appear in isolated environment
    assert!(
        !stdout.contains("SECRET_API_KEY=super_secret_value"),
        "Secret API key leaked into high isolation environment!"
    );

    // XDG_RUNTIME_DIR must be blocked (D-Bus / Wayland socket exposure)
    assert!(
        !stdout.contains("XDG_RUNTIME_DIR=/run/user/9999"),
        "XDG_RUNTIME_DIR leaked into high isolation environment!"
    );

    // PATH must still be present for basic functionality
    assert!(
        stdout.contains("PATH="),
        "PATH is missing from high isolation environment"
    );

    // Clean up
    std::env::remove_var("SECRET_API_KEY");
    std::env::remove_var("XDG_RUNTIME_DIR");
}
