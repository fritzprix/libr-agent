use crate::models::workspace_isolation::{
    validate_env_key, validate_env_value, WorkspaceIsolationMode,
};
use crate::repositories::SessionMetadata;
use crate::session_isolation::{PathMappingLayer, ShellDialect, ShellType, SpawnedShell};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use thiserror::Error;
use tokio::process::Command as AsyncCommand;
use tokio::sync::Mutex;

static DOCKER_SESSION_LOCKS: Lazy<DashMap<String, Arc<Mutex<()>>>> = Lazy::new(DashMap::new);

type RuntimeResult<T> = Result<T, WorkspaceRuntimeError>;

#[derive(Debug, Error)]
pub enum WorkspaceRuntimeError {
    #[error("Docker is not available. Ensure Docker Desktop/daemon is running and the current user can run Docker without sudo. Details: {0}")]
    DockerNotAvailable(String),
    #[error("Docker command failed: {0}")]
    DockerCommandFailed(String),
    #[error("Docker container '{container_name}' is not owned by session '{session_id}'")]
    OwnershipMismatch {
        container_name: String,
        session_id: String,
    },
    #[error("dockerConfig is required for Docker workspace isolation")]
    MissingDockerConfig,
    #[error("Missing Docker container name for session {0}")]
    MissingContainerName(String),
    #[error("Missing Docker host workspace path for session {0}")]
    MissingHostWorkspacePath(String),
    #[error("Invalid Docker workspace config: {0}")]
    InvalidConfig(String),
    #[error("Docker workspace path is not valid UTF-8: {0}")]
    InvalidWorkspacePath(String),
    #[error("Docker image for session {session_id} must include bash for shell execution. Details: {details}")]
    BashUnavailable { session_id: String, details: String },
    #[error(
        "Docker image for session {session_id} must include bash or POSIX sh for shell execution"
    )]
    ShellUnavailable { session_id: String },
    #[error("Docker host port {0} is already in use on 127.0.0.1")]
    HostPortUnavailable(u16),
    #[error("{0}")]
    Io(String),
}

pub struct WorkspaceRuntimeManager;

impl WorkspaceRuntimeManager {
    pub async fn healthcheck() -> RuntimeResult<()> {
        run_docker_status(["--version"]).await?;
        run_docker_status(["info"])
            .await
            .map_err(|error| WorkspaceRuntimeError::DockerNotAvailable(error.to_string()))?;
        Ok(())
    }

    pub async fn ensure_runtime(session: &SessionMetadata) -> RuntimeResult<()> {
        if session.workspace_isolation != WorkspaceIsolationMode::Docker {
            return Ok(());
        }

        let lock = DOCKER_SESSION_LOCKS
            .entry(session.id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        let _guard = lock.lock().await;

        Self::healthcheck().await?;

        let container_name = docker_container_name(session)?;
        let host_workspace = docker_host_workspace_path(session)?;
        ensure_bash_image_contract(&session.id, &container_name, session, &host_workspace).await
    }

    pub async fn create_docker_exec_command(
        session: &SessionMetadata,
        command: &str,
        env_vars: &std::collections::HashMap<String, String>,
    ) -> RuntimeResult<AsyncCommand> {
        Self::ensure_runtime(session).await?;

        let container_name = docker_container_name(session)?;
        let mut cmd = AsyncCommand::new("docker");
        apply_docker_cli_env(&mut cmd);
        cmd.args(["exec", "-i", "-w", "/workspace"]);

        let mut merged_env = session
            .docker_config
            .as_ref()
            .map(|config| config.env.clone())
            .unwrap_or_default();
        for (key, value) in env_vars {
            validate_env_key(key).map_err(WorkspaceRuntimeError::InvalidConfig)?;
            validate_env_value(key, value).map_err(WorkspaceRuntimeError::InvalidConfig)?;
            merged_env.insert(key.clone(), value.clone());
        }

        for (key, value) in merged_env {
            validate_env_key(&key).map_err(WorkspaceRuntimeError::InvalidConfig)?;
            validate_env_value(&key, &value).map_err(WorkspaceRuntimeError::InvalidConfig)?;
            cmd.arg("-e").arg(format!("{key}={value}"));
        }

        let shell = docker_shell_for_session(session).await?;
        cmd.arg(container_name);
        // runShell intentionally executes shell syntax; this mirrors the host shell path.
        cmd.args([shell.command(), "-lc", command]);
        Ok(cmd)
    }

    pub async fn spawn_docker_persistent_shell(
        session: &SessionMetadata,
    ) -> RuntimeResult<SpawnedShell> {
        Self::ensure_runtime(session).await?;

        let container_name = docker_container_name(session)?;
        let host_workspace = docker_host_workspace_path(session)?;
        let mut cmd = AsyncCommand::new("docker");
        apply_docker_cli_env(&mut cmd);
        cmd.args(["exec", "-i", "-w", "/workspace"]);

        if let Some(config) = &session.docker_config {
            for (key, value) in &config.env {
                validate_env_key(key).map_err(WorkspaceRuntimeError::InvalidConfig)?;
                validate_env_value(key, value).map_err(WorkspaceRuntimeError::InvalidConfig)?;
                cmd.arg("-e").arg(format!("{key}={value}"));
            }
        }

        let shell = docker_shell_for_session(session).await?;
        cmd.arg(container_name);
        match shell {
            ShellType::Bash => cmd.args(["bash", "--norc", "--noprofile"]),
            ShellType::Sh => cmd.arg("sh"),
            ShellType::PowerShell => unreachable!("Docker Unix shell cannot be PowerShell"),
        };
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd.spawn().map_err(|e| {
            WorkspaceRuntimeError::Io(format!("Failed to spawn Docker persistent shell: {e}"))
        })?;

        Ok(SpawnedShell {
            child,
            initial_cwd: "/workspace".to_string(),
            path_mapper: PathMappingLayer::new(host_workspace),
            shell_type: shell,
            shell_dialect: match shell {
                ShellType::Bash => ShellDialect::Bash,
                ShellType::Sh => ShellDialect::Sh,
                ShellType::PowerShell => unreachable!("Docker Unix shell cannot be PowerShell"),
            },
        })
    }

    pub async fn remove_runtime_for_session(session: &SessionMetadata) -> RuntimeResult<()> {
        if session.workspace_isolation != WorkspaceIsolationMode::Docker {
            return Ok(());
        }

        Self::healthcheck().await?;
        let container_name = docker_container_name(session)?;
        if !docker_container_exists(&container_name).await? {
            return Ok(());
        }

        verify_container_label(&container_name, &session.id).await?;
        run_docker_status(["rm", "-f", "-v", &container_name]).await
    }

    pub async fn sweep_stale_containers(
        active_session_ids: &HashSet<String>,
    ) -> RuntimeResult<Vec<String>> {
        Self::healthcheck().await?;

        let output = docker_output_slice(&[
            "ps",
            "-a",
            "--filter",
            "label=com.libragent.session_id",
            "--format",
            "{{.Names}}",
        ])
        .await?;

        let mut removed = Vec::new();
        for container_name in output
            .lines()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            let session_id = docker_container_label(container_name).await?;
            if active_session_ids.contains(&session_id) {
                continue;
            }

            verify_container_label(container_name, &session_id).await?;
            if let Err(error) = run_docker_status(["rm", "-f", "-v", container_name]).await {
                tracing::warn!(
                    "Failed to remove stale Docker container '{}': {}",
                    container_name,
                    error
                );
                continue;
            }
            removed.push(container_name.to_string());
        }

        Ok(removed)
    }
}

async fn ensure_bash_image_contract(
    session_id: &str,
    container_name: &str,
    session: &SessionMetadata,
    host_workspace: &Path,
) -> RuntimeResult<()> {
    if docker_container_exists(container_name).await? {
        verify_container_label(container_name, session_id).await?;
        if !docker_container_running(container_name).await? {
            run_docker_status(["start", container_name]).await?;
        }
    } else {
        let config = session
            .docker_config
            .as_ref()
            .ok_or(WorkspaceRuntimeError::MissingDockerConfig)?;
        config
            .validate()
            .map_err(WorkspaceRuntimeError::InvalidConfig)?;

        tokio::fs::create_dir_all(host_workspace)
            .await
            .map_err(|e| {
                WorkspaceRuntimeError::Io(format!(
                    "Failed to create Docker host workspace '{}': {e}",
                    host_workspace.display()
                ))
            })?;

        let mount = format!("{}:/workspace", docker_mount_path(host_workspace)?);
        let label = format!("com.libragent.session_id={session_id}");
        let mut cmd = AsyncCommand::new("docker");
        apply_docker_cli_env(&mut cmd);
        cmd.args([
            "run",
            "-d",
            "--name",
            container_name,
            "--label",
            &label,
            "-v",
            &mount,
            "-w",
            "/workspace",
        ]);

        if let Some(config) = &session.docker_config {
            append_port_binding_args(&mut cmd, config).await?;
        }

        if let Some(user) = current_uid_gid().await {
            cmd.args(["--user", &user]);
        }

        cmd.args([&config.image, "tail", "-f", "/dev/null"]);
        run_status_command(cmd).await?;
    }

    verify_container_label(container_name, session_id).await?;
    ensure_supported_shell(session_id, container_name)
        .await
        .map(|_| ())
}

async fn append_port_binding_args(
    cmd: &mut AsyncCommand,
    config: &crate::models::workspace_isolation::DockerWorkspaceConfig,
) -> RuntimeResult<()> {
    for binding in &config.port_bindings {
        if let Some(host_port) = binding.host_port {
            ensure_host_port_available(host_port)?;
        }

        let published = match binding.host_port {
            Some(host_port) => format!("127.0.0.1:{host_port}:{}", binding.container_port),
            None => format!("127.0.0.1::{}", binding.container_port),
        };
        cmd.arg("-p").arg(published);
    }

    Ok(())
}

fn ensure_host_port_available(port: u16) -> RuntimeResult<()> {
    TcpListener::bind(("127.0.0.1", port))
        .map(drop)
        .map_err(|_| WorkspaceRuntimeError::HostPortUnavailable(port))
}

async fn docker_shell_for_session(session: &SessionMetadata) -> RuntimeResult<ShellType> {
    let container_name = docker_container_name(session)?;
    ensure_supported_shell(&session.id, &container_name).await
}

async fn ensure_supported_shell(
    session_id: &str,
    container_name: &str,
) -> RuntimeResult<ShellType> {
    if run_docker_status(["exec", container_name, "bash", "-lc", "true"])
        .await
        .is_ok()
    {
        return Ok(ShellType::Bash);
    }

    if run_docker_status(["exec", container_name, "sh", "-lc", "true"])
        .await
        .is_ok()
    {
        return Ok(ShellType::Sh);
    }

    Err(WorkspaceRuntimeError::ShellUnavailable {
        session_id: session_id.to_string(),
    })
}

async fn docker_container_exists(container_name: &str) -> RuntimeResult<bool> {
    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args(["inspect", container_name]);
    let output = cmd.output().await.map_err(|e| {
        WorkspaceRuntimeError::DockerCommandFailed(format!(
            "Failed to inspect Docker container {container_name}: {e}"
        ))
    })?;
    Ok(output.status.success())
}

async fn docker_container_running(container_name: &str) -> RuntimeResult<bool> {
    let output = docker_output(["inspect", "-f", "{{.State.Running}}", container_name]).await?;
    Ok(output.trim() == "true")
}

async fn verify_container_label(container_name: &str, session_id: &str) -> RuntimeResult<()> {
    let label = docker_container_label(container_name).await?;

    if label.trim() != session_id {
        return Err(WorkspaceRuntimeError::OwnershipMismatch {
            container_name: container_name.to_string(),
            session_id: session_id.to_string(),
        });
    }

    Ok(())
}

async fn docker_container_label(container_name: &str) -> RuntimeResult<String> {
    docker_output([
        "inspect",
        "-f",
        "{{ index .Config.Labels \"com.libragent.session_id\" }}",
        container_name,
    ])
    .await
}

fn docker_container_name(session: &SessionMetadata) -> RuntimeResult<String> {
    session
        .docker_container_name
        .clone()
        .or_else(|| Some(format!("libragent-session-{}", session.id)))
        .ok_or_else(|| WorkspaceRuntimeError::MissingContainerName(session.id.clone()))
}

fn docker_host_workspace_path(session: &SessionMetadata) -> RuntimeResult<PathBuf> {
    session
        .docker_host_workspace_path
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| WorkspaceRuntimeError::MissingHostWorkspacePath(session.id.clone()))
}

fn docker_mount_path(path: &Path) -> RuntimeResult<String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| WorkspaceRuntimeError::InvalidWorkspacePath(path.display().to_string()))
}

#[cfg(unix)]
async fn current_uid_gid() -> Option<String> {
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
async fn current_uid_gid() -> Option<String> {
    None
}

async fn run_docker_status<const N: usize>(args: [&str; N]) -> RuntimeResult<()> {
    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args(args);
    run_status_command(cmd).await
}

async fn docker_output<const N: usize>(args: [&str; N]) -> RuntimeResult<String> {
    docker_output_slice(&args).await
}

async fn docker_output_slice(args: &[&str]) -> RuntimeResult<String> {
    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args(args);
    let output = cmd
        .output()
        .await
        .map_err(|e| WorkspaceRuntimeError::DockerCommandFailed(e.to_string()))?;
    if !output.status.success() {
        return Err(format_docker_failure(&output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn run_status_command(mut cmd: AsyncCommand) -> RuntimeResult<()> {
    let output = cmd
        .output()
        .await
        .map_err(|e| WorkspaceRuntimeError::DockerCommandFailed(e.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format_docker_failure(&output))
    }
}

fn format_docker_failure(output: &std::process::Output) -> WorkspaceRuntimeError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let details = if !stderr.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };
    WorkspaceRuntimeError::DockerCommandFailed(format!(
        "docker exited with status {}: {details}",
        output.status
    ))
}

fn apply_docker_cli_env(cmd: &mut AsyncCommand) {
    crate::utils::env::apply_isolated_env_async(cmd);
}
