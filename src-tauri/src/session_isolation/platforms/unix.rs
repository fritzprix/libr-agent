use crate::session_isolation::common::get_shell_command;
use crate::session_isolation::types::{IsolatedProcessConfig, IsolationConfig};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use tokio::process::Command as AsyncCommand;
use tracing::{info, warn};

/// Basic isolation: environment variables and working directory
pub async fn create_basic_isolated_command(
    config: IsolatedProcessConfig,
) -> Result<AsyncCommand, String> {
    // Unix-like logic from original create_basic_isolated_command
    let shell_cmd = get_shell_command(None);
    let mut cmd = AsyncCommand::new(shell_cmd);

    // Set working directory
    cmd.current_dir(&config.workspace_path);

    // Unix: Inherit environment variables (do not clear)
    cmd.env("HOME", &config.workspace_path);
    cmd.env("PWD", &config.workspace_path);
    cmd.env("TMPDIR", config.workspace_path.join("tmp"));
    // PATH is inherited from parent process (agent)

    // Add user-specified environment variables (applies to all platforms)
    for (key, value) in &config.env_vars {
        cmd.env(key, value);
    }

    // Unix shells (bash, sh) use -c flag
    if config.args.is_empty() {
        // No arguments: we can safely run the command string directly with -c
        let script = config.command.clone();
        info!(
            "Unix shell execution (no args): {} -c {}",
            shell_cmd, script
        );
        cmd.args(["-c", &script]);
    } else {
        // Arguments present: use `sh -c 'cmd "$@"' cmd arg1 arg2...` pattern to preserve boundaries
        let script = format!("{} \"$@\"", config.command);
        let mut shell_args: Vec<String> = Vec::new();
        shell_args.push("-c".to_string());
        shell_args.push(script);
        // First argument after the script becomes $0 inside the shell
        shell_args.push(config.command.clone());
        // Remaining arguments become $1, $2, ...; they are expanded via "$@" without word splitting
        shell_args.extend(config.args.clone());

        info!(
            "Unix shell execution with args: {} {:?}",
            shell_cmd, shell_args
        );
        cmd.args(shell_args);
    }

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
