#![cfg(target_os = "macos")]

use tokio::process::Command as AsyncCommand;
use tracing::{info, warn};
use crate::session_isolation::types::IsolatedProcessConfig;
use crate::session_isolation::common::is_command_available;

/// macOS high isolation using sandbox-exec
pub async fn create_high_isolation(
    config: &IsolatedProcessConfig,
) -> Result<Option<AsyncCommand>, String> {
    // Check if sandbox-exec is available
    if !is_command_available("sandbox-exec").await {
        warn!("sandbox-exec not available, falling back to medium isolation");
        return Ok(None);
    }

    // Create a sandbox profile for this session
    let profile_content = create_macos_sandbox_profile(config)?;
    let profile_path = config.workspace_path.join(".sandbox_profile");

    tokio::fs::write(&profile_path, profile_content)
        .await
        .map_err(|e| format!("Failed to write sandbox profile: {e}"))?;

    let mut cmd = AsyncCommand::new("sandbox-exec");
    cmd.args([
        "-f",
        profile_path
            .to_str()
            .ok_or_else(|| "Failed to convert profile path to string".to_string())?,
    ]);
    cmd.arg(&config.command);
    cmd.args(&config.args);

    // Set environment and working directory
    cmd.current_dir(&config.workspace_path);
    cmd.env("HOME", &config.workspace_path);
    cmd.env("PWD", &config.workspace_path);
    // PATH and other envs inherited

    for (key, value) in &config.env_vars {
        cmd.env(key, value);
    }

    info!(
        "Created macOS high isolation command for session: {}",
        config.session_id
    );
    Ok(Some(cmd))
}

/// Create macOS sandbox profile
fn create_macos_sandbox_profile(
    config: &IsolatedProcessConfig,
) -> Result<String, String> {
    let workspace_path_str = config
        .workspace_path
        .to_str()
        .ok_or("Invalid workspace path")?;

    let profile = format!(
        r#"
(version 1)
(deny default)

;; Allow basic system operations
(allow process-info* (target self))
(allow signal (target self))
(allow sysctl-read)

;; Allow reading system frameworks and libraries
(allow file-read*
    (subpath "/System/Library")
    (subpath "/usr/lib")
    (subpath "/usr/bin")
    (subpath "/bin"))

;; Allow access to workspace directory
(allow file-read* file-write* file-ioctl
    (subpath "{workspace_path}"))

;; Allow temporary directory access
(allow file-read* file-write* file-ioctl
    (subpath "/tmp")
    (subpath "/var/tmp"))

;; Allow network access if enabled
{network_rules}

;; Deny access to sensitive directories
(deny file-read* file-write*
    (subpath "/private")
    (subpath "$HOME" (except (subpath "{workspace_path}"))))
"#,
        workspace_path = workspace_path_str,
        network_rules = "(allow network*)" // Allow network access by default
    );

    Ok(profile)
}
