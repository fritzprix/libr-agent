use crate::session_isolation::platforms::unix::create_medium_isolated_command;
use crate::session_isolation::types::{IsolatedProcessConfig, IsolationConfig};
use tokio::process::Command as AsyncCommand;
use tracing::{info, warn};

/// Linux high isolation using unshare (user namespaces)
pub async fn create_high_isolated_command(
    config: IsolatedProcessConfig,
    isolation_config: &IsolationConfig,
) -> Result<AsyncCommand, String> {
    // Probe whether unprivileged user namespaces are actually usable, not just
    // whether the binary exists.  On many CI/container environments `unshare` is
    // present but the kernel has user-namespace creation restricted, so the binary
    // would exit with an error and the child `env` would never run.
    // Probe with the exact flags we intend to use so that a capability that
    // works without --mount but fails with it (e.g. no CAP_SYS_ADMIN in a
    // container) causes us to fall back rather than silently producing empty
    // output when the real command runs.
    let mut userns_check = AsyncCommand::new("unshare");
    userns_check.env_clear();
    for (k, v) in crate::utils::env::get_isolated_env() {
        userns_check.env(k, v);
    }

    let userns_available = userns_check
        .args(["--user", "--pid", "--mount", "--fork", "--", "true"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !userns_available {
        warn!("user namespaces not available or unprivileged, falling back to medium isolation");
        return create_medium_isolated_command(config, isolation_config).await;
    }

    let mut cmd = AsyncCommand::new("unshare");

    // Configure namespaces for isolation
    cmd.args([
        "--user",  // User namespace isolation
        "--pid",   // PID namespace isolation
        "--mount", // Mount namespace isolation
        "--fork",  // Fork before exec
        "--",
    ]);

    // Add the actual command
    cmd.arg(&config.command);
    cmd.args(&config.args);

    // Set environment and working directory
    cmd.current_dir(&config.workspace_path);

    // Apply environment isolation
    cmd.env_clear();
    for (k, v) in crate::utils::env::get_isolated_env() {
        cmd.env(k, v);
    }

    // Always ensure PATH is present so basic commands work in the isolated environment.
    // Fall back to a safe, minimal default if the parent process has no PATH.
    let path_value =
        std::env::var("PATH").unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin".to_string());
    cmd.env("PATH", path_value);

    cmd.env("HOME", &config.workspace_path);
    cmd.env("PWD", &config.workspace_path);

    for (key, value) in &config.env_vars {
        cmd.env(key, value);
    }

    info!(
        "Created Linux high isolation command for session: {}",
        config.session_id
    );
    Ok(cmd)
}
