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

    // FIX: Clear environment to avoid leaking secrets from parent process
    cmd.env_clear();

    // Re-inherit only safe/necessary variables
    // PATH is critical for finding binaries
    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", path);
    }
    // TERM is needed for proper output formatting (colors, etc.)
    if let Ok(term) = std::env::var("TERM") {
        cmd.env("TERM", term);
    }
    // USER is often useful for scripts
    if let Ok(user) = std::env::var("USER") {
        cmd.env("USER", user);
    }
    // LOGNAME is often useful for scripts
    if let Ok(logname) = std::env::var("LOGNAME") {
        cmd.env("LOGNAME", logname);
    }

    // Set working directory
    cmd.current_dir(&config.workspace_path);

    // Unix: Explicitly set safe environment variables
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
    use crate::session_isolation::types::IsolationLevel;

    #[tokio::test]
    async fn test_create_basic_isolated_command_clears_env() {
        // This test verifies that sensitive environment variables are cleared
        // while necessary ones (like PATH) are preserved.

        let config = IsolatedProcessConfig {
            session_id: "test-session".to_string(),
            workspace_path: std::env::temp_dir(),
            command: "env".to_string(), // Just print env
            args: vec![],
            env_vars: HashMap::new(),
            isolation_level: IsolationLevel::Basic,
            shell_type: None,
        };

        let mut cmd = create_basic_isolated_command(config).await.expect("failed to create command");

        // Execute the command and capture output
        let output = cmd.output().await.expect("failed to execute command");
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Assertions
        // 1. CARGO_MANIFEST_DIR should be present in the PARENT (test runner) environment
        assert!(std::env::var("CARGO_MANIFEST_DIR").is_ok(), "Parent env should have CARGO_MANIFEST_DIR");

        // 2. Child environment should NOT have CARGO_MANIFEST_DIR (it's a secret/internal var)
        // If this fails, it means env leakage is happening!
        assert!(!stdout.contains("CARGO_MANIFEST_DIR="), "Environment leakage detected! CARGO_MANIFEST_DIR found in child process: {}", stdout);

        // 3. Child environment SHOULD have PATH (necessary for finding binaries)
        assert!(stdout.contains("PATH="), "PATH environment variable missing in child process");

        // 4. Child environment SHOULD have HOME set to workspace
        // We set it explicitly, so it should be there.
        // Note: temp_dir() might be resolved to a canonical path, so we check loosely or check if it's present.
        assert!(stdout.contains("HOME="), "HOME environment variable missing in child process");
    }
}
