#![cfg(target_os = "linux")]

use tokio::process::Command as AsyncCommand;
use tracing::{info, warn};
use crate::session_isolation::types::IsolatedProcessConfig;
use crate::session_isolation::common::is_command_available;

/// Linux high isolation using unshare (user namespaces)
pub async fn create_high_isolation(
    config: &IsolatedProcessConfig,
) -> Result<Option<AsyncCommand>, String> {
    // Check if unshare is available
    if !is_command_available("unshare").await {
        warn!("unshare not available, falling back to medium isolation");
        return Ok(None);
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
    cmd.env("HOME", &config.workspace_path);
    cmd.env("PWD", &config.workspace_path);
    // PATH and other envs inherited (if unshare allows, though user namespaces might be tricky)

    for (key, value) in &config.env_vars {
        cmd.env(key, value);
    }

    info!(
        "Created Linux high isolation command for session: {}",
        config.session_id
    );
    Ok(Some(cmd))
}
