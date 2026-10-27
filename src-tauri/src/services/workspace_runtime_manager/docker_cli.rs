use tokio::process::Command as AsyncCommand;

use super::error::{
    format_docker_failure, map_docker_spawn_error, RuntimeResult, WorkspaceRuntimeError,
};

/// Normalize Docker GOARCH / inspect Architecture values to Rust `std::env::consts::ARCH` style.
pub fn normalize_docker_arch(arch: &str) -> String {
    match arch.trim().to_ascii_lowercase().as_str() {
        "amd64" | "x86_64" | "x64" => "x86_64".to_string(),
        "arm64" | "aarch64" => "aarch64".to_string(),
        "arm" | "armhf" | "armv7" | "armv7l" => "arm".to_string(),
        "i386" | "i686" | "386" => "x86".to_string(),
        "" => std::env::consts::ARCH.to_string(),
        other => other.to_string(),
    }
}

pub(super) async fn docker_container_architecture(container_name: &str) -> RuntimeResult<String> {
    let arch = docker_output(["inspect", "-f", "{{.Architecture}}", container_name]).await?;
    Ok(normalize_docker_arch(&arch))
}

pub(super) async fn docker_container_exists(container_name: &str) -> RuntimeResult<bool> {
    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args(["inspect", container_name]);
    let output = cmd.output().await.map_err(map_docker_spawn_error)?;
    Ok(output.status.success())
}

pub(super) async fn docker_container_running(container_name: &str) -> RuntimeResult<bool> {
    let output = docker_output(["inspect", "-f", "{{.State.Running}}", container_name]).await?;
    Ok(output.trim() == "true")
}

pub(super) async fn verify_container_label(
    container_name: &str,
    session_id: &str,
) -> RuntimeResult<()> {
    let label = docker_container_label(container_name).await?;

    if label.trim() != session_id {
        return Err(WorkspaceRuntimeError::OwnershipMismatch {
            container_name: container_name.to_string(),
            session_id: session_id.to_string(),
        });
    }

    Ok(())
}

pub(super) async fn docker_container_label(container_name: &str) -> RuntimeResult<String> {
    docker_output([
        "inspect",
        "-f",
        "{{ index .Config.Labels \"com.libragent.session_id\" }}",
        container_name,
    ])
    .await
}

pub(super) async fn docker_container_socket_generation(
    container_name: &str,
) -> RuntimeResult<Option<String>> {
    let output = docker_output([
        "inspect",
        "-f",
        "{{ index .Config.Labels \"com.libragent.socket_generation\" }}",
        container_name,
    ])
    .await?;
    let trimmed = output.trim();
    if trimmed.is_empty() || trimmed == "<no value>" {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

#[cfg(unix)]
pub(super) async fn current_uid_gid() -> Option<String> {
    let uid = AsyncCommand::new("id")
        .arg("-u")
        .output()
        .await
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())?;
    let gid = AsyncCommand::new("id")
        .arg("-g")
        .output()
        .await
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())?;

    Some(format!("{uid}:{gid}"))
}

#[cfg(not(unix))]
pub(super) async fn current_uid_gid() -> Option<String> {
    None
}

pub(super) async fn run_docker_status<const N: usize>(args: [&str; N]) -> RuntimeResult<()> {
    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args(args);
    run_status_command(cmd).await
}

pub(super) async fn docker_output<const N: usize>(args: [&str; N]) -> RuntimeResult<String> {
    docker_output_slice(&args).await
}

pub(super) async fn docker_output_slice(args: &[&str]) -> RuntimeResult<String> {
    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args(args);
    let output = cmd.output().await.map_err(map_docker_spawn_error)?;
    if !output.status.success() {
        return Err(format_docker_failure(&output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(super) async fn run_status_command(mut cmd: AsyncCommand) -> RuntimeResult<()> {
    let output = cmd.output().await.map_err(map_docker_spawn_error)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format_docker_failure(&output))
    }
}

pub(super) fn apply_docker_cli_env(cmd: &mut AsyncCommand) {
    // On Windows GUI hosts, docker.exe is a console subsystem binary; without this
    // flag every runShell/docker exec flashes a terminal window.
    crate::utils::platform::suppress_console_window_async(cmd);
    crate::utils::env::apply_isolated_env_async(cmd);
}
