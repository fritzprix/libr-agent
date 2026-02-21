use crate::session_isolation::common::get_shell_command;
use crate::session_isolation::types::{IsolatedProcessConfig, IsolationConfig};
use tokio::process::Command as AsyncCommand;
use tracing::{info, warn};

/// Basic isolation: environment variables and working directory
pub async fn create_basic_isolated_command(
    config: IsolatedProcessConfig,
) -> Result<AsyncCommand, String> {
    // Unix-like logic from original create_basic_isolated_command
    let shell_cmd = get_shell_command(None);
    let mut cmd = AsyncCommand::new(shell_cmd);

    // Clear inherited environment variables to prevent leakage
    cmd.env_clear();

    // Set working directory
    cmd.current_dir(&config.workspace_path);

    // Explicitly inherit PATH and TERM from parent process if available
    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", path);
    }
    if let Ok(term) = std::env::var("TERM") {
        cmd.env("TERM", term);
    }

    // Unix: Set specific environment variables
    cmd.env("HOME", &config.workspace_path);
    cmd.env("PWD", &config.workspace_path);
    cmd.env("TMPDIR", config.workspace_path.join("tmp"));
    // Force English output for consistent AI reasoning
    cmd.env("LC_ALL", "en_US.UTF-8");
    cmd.env("LANG", "en_US.UTF-8");

    // Add user-specified environment variables (applies to all platforms)
    for (key, value) in &config.env_vars {
        cmd.env(key, value);
    }

    // Unix shells (bash, sh) use -c flag.
    // Instead of string concatenation which is vulnerable to injection if args contain metacharacters,
    // we use the `sh -c "$@"` pattern to pass arguments safely.
    // Syntax: sh -c 'command "$@"' -- arg0 arg1 arg2...
    // where arg0 becomes $0 (typically script name), arg1 becomes $1, etc.

    // We set $0 to the command name itself for better `ps` output visibility if supported,
    // or just a placeholder. Let's use config.command as $0.

    // Command string: simply execute "$@" which expands to all positional parameters preserving quoting.
    // Wait, "$@" expands to $1 $2... it does NOT include $0.
    // So if we run `sh -c '"$@"' -- cmd arg1`, it runs `arg1`!

    let mut shell_args = vec!["-c"];

    if config.args.is_empty() {
        // Run as a script: sh -c "ls -l"
        shell_args.push(&config.command);
    } else {
        // Run as executable + args safely: sh -c '"$0" "$@"' exe arg1 arg2
        shell_args.push("\"$0\" \"$@\"");
        shell_args.push(&config.command); // $0
        for arg in &config.args {
            shell_args.push(arg); // $1, $2...
        }
    }

    info!(
        "Unix shell execution: {} {:?} (original: '{}' args: {:?})",
        shell_cmd, shell_args, config.command, config.args
    );
    cmd.args(shell_args);

    info!(
        "Isolated command created for session {} with isolation level {:?}",
        config.session_id, config.isolation_level
    );
    Ok(cmd)
}

/// Medium isolation: process groups + resource limits
pub async fn create_medium_isolated_command(
    config: IsolatedProcessConfig,
    isolation_config: &IsolationConfig,
) -> Result<AsyncCommand, String> {
    let mut cmd = create_basic_isolated_command(config.clone()).await?;

    // Apply platform-specific process group isolation
    #[cfg(unix)]
    {
        cmd.process_group(0); // Create new process group
    }

    // Apply resource limits (Logging only for now as per original code)
    let limits = &isolation_config.resource_limits;
    info!(
        "Resource limits configured: memory_mb={:?}, time_secs={:?}, open_files={:?}",
        limits.max_memory_mb, limits.max_execution_time_secs, limits.max_open_files
    );

    Ok(cmd)
}

/// High isolation fallback for generic Unix: medium isolation + warning
#[allow(dead_code)] // Unused on Linux/macOS as they have specific implementations
pub async fn create_high_isolated_command(
    config: IsolatedProcessConfig,
    isolation_config: &IsolationConfig,
) -> Result<AsyncCommand, String> {
    warn!("High isolation not supported on this platform, falling back to medium isolation");
    create_medium_isolated_command(config, isolation_config).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_basic_isolation_clears_env() {
        // Set a secret in the parent process
        let secret_key = "SENTINEL_TEST_SECRET";
        let secret_val = "super_secret_value";
        // SAFETY: Only safe if tests run sequentially or don't rely on clean env
        unsafe { std::env::set_var(secret_key, secret_val) };

        let dir = tempdir().unwrap();
        let config = IsolatedProcessConfig {
            session_id: "test-session".to_string(),
            workspace_path: dir.path().to_path_buf(),
            command: "env".to_string(),
            args: vec![],
            env_vars: HashMap::new(),
            isolation_level: crate::session_isolation::IsolationLevel::Basic,
            shell_type: None,
        };

        // Create the command
        let mut cmd = create_basic_isolated_command(config).await.expect("Failed to create command");

        // Execute and capture output
        let output = cmd.output().await.expect("Failed to execute command");
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Clean up env var
        unsafe { std::env::remove_var(secret_key) };

        // Assertions
        // Current implementation DOES NOT clear env, so this assertion SHOULD FAIL if the test is correct for reproduction.
        // We assert what SHOULD happen (clearing), so we expect the test to fail now.
        assert!(!stdout.contains(secret_key), "Environment variable should be cleared");

        // These should be present
        // PATH might be missing if we cleared it without restoring, but currently it's inherited.
        // Once we fix it, we must ensure PATH is restored.
        // In this reproduction step, PATH is inherited, so it should be there.
        assert!(stdout.contains("PATH="), "PATH should be preserved/restored");
        assert!(stdout.contains("HOME="), "HOME should be set");
    }
}
