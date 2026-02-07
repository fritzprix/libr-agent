use tokio::process::Command as AsyncCommand;
use tracing::{info, warn};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use crate::session_isolation::types::{IsolatedProcessConfig, IsolationConfig};
use crate::session_isolation::common::get_shell_command;

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

    // Correct pattern: `sh -c '"$0" "$@"' command arg1 arg2`
    // Then $0 = command, $1 = arg1, $2 = arg2.
    // Expansion `"$0" "$@"` becomes `command arg1 arg2`.

    let shell_script = "\"$0\" \"$@\"";

    let mut shell_args = Vec::new();
    shell_args.push("-c");
    shell_args.push(shell_script);
    shell_args.push("--"); // Delimiter (optional but good practice)
    shell_args.push(&config.command); // This becomes $0
    for arg in &config.args {
        shell_args.push(arg); // These become $1, $2...
    }

    info!("Unix shell execution: {} -c '{}' -- {} {:?}", shell_cmd, shell_script, config.command, config.args);
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
