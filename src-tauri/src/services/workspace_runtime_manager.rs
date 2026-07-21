use std::collections::HashSet;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;

use crate::models::workspace_isolation::{
    validate_env_key, validate_env_value, WorkspaceIsolationMode, DEFAULT_DOCKER_WORKDIR,
};
use crate::repositories::{SessionMetadata, SessionRepository};
use crate::session_isolation::{PathMappingLayer, ShellDialect, ShellType, SpawnedShell};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use thiserror::Error;
use tokio::process::Command as AsyncCommand;
use tokio::sync::{Mutex, Notify};

// ── Statics ──────────────────────────────────────────────────────────────────

/// Per-session mutex to serialize container creation and health-check attempts.
static DOCKER_SESSION_LOCKS: Lazy<DashMap<String, Arc<Mutex<()>>>> = Lazy::new(DashMap::new);

/// Health-check cache: session_id → (last_check_time, is_healthy).
/// Avoids running `docker --version` + `docker info` on every shell command.
static DOCKER_HEALTH_CACHE: Lazy<DashMap<String, (Instant, bool)>> = Lazy::new(DashMap::new);

/// Container readiness cache: session_id → last verified time.
static DOCKER_CONTAINER_READY_CACHE: Lazy<DashMap<String, Instant>> = Lazy::new(DashMap::new);

/// Resolved shell type cache: session_id → bash/sh.
static DOCKER_SHELL_CACHE: Lazy<DashMap<String, ShellType>> = Lazy::new(DashMap::new);

/// Resolved container architecture cache: session_id → Rust-style arch (x86_64/aarch64/…).
static DOCKER_ARCH_CACHE: Lazy<DashMap<String, String>> = Lazy::new(DashMap::new);

/// Waiters notified when background Docker provisioning completes for a session.
static DOCKER_PROVISIONING_WAITERS: Lazy<DashMap<String, Arc<Notify>>> = Lazy::new(DashMap::new);

/// Tracks sessions with an in-flight provisioning task to avoid duplicate spawns.
static DOCKER_PROVISIONING_IN_FLIGHT: Lazy<DashMap<String, ()>> = Lazy::new(DashMap::new);

const DOCKER_RUNTIME_CACHE_TTL_SECS: u64 = 30;

pub type DockerStepReporter = Arc<dyn Fn(&str) + Send + Sync>;

// ── Types ────────────────────────────────────────────────────────────────────

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

/// Prefix for structured errors surfaced to the frontend agent layer.
pub const AGENT_ERROR_DOCKER_NOT_AVAILABLE: &str = "DOCKER_NOT_AVAILABLE:";

impl WorkspaceRuntimeError {
    /// Converts runtime errors into agent-facing strings with stable machine-readable codes.
    pub fn to_agent_string(self) -> String {
        match self {
            Self::DockerNotAvailable(_) => {
                format!("{} {}", AGENT_ERROR_DOCKER_NOT_AVAILABLE, self)
            }
            other => other.to_string(),
        }
    }
}

// ── Manager ──────────────────────────────────────────────────────────────────

pub struct WorkspaceRuntimeManager;

impl WorkspaceRuntimeManager {
    pub async fn healthcheck() -> RuntimeResult<()> {
        run_docker_status(["--version"]).await?;
        run_docker_status(["info"])
            .await
            .map_err(|error| WorkspaceRuntimeError::DockerNotAvailable(error.to_string()))?;
        Ok(())
    }

    /// Ensures the Docker runtime is healthy and the session container exists.
    /// Acquires a per-session lock to serialize container creation.
    pub async fn ensure_runtime(session: &SessionMetadata) -> RuntimeResult<()> {
        wait_for_docker_provisioning(&session.id).await;
        prepare_docker_runtime(session, false, false, None)
            .await
            .map(|_| ())
    }

    /// Returns the cached Docker shell for a session, if runtime has been prepared.
    pub fn cached_docker_shell(session_id: &str) -> Option<ShellType> {
        DOCKER_SHELL_CACHE.get(session_id).map(|entry| *entry)
    }

    /// Returns the cached Docker container architecture for a session, if known.
    pub fn cached_docker_arch(session_id: &str) -> Option<String> {
        DOCKER_ARCH_CACHE.get(session_id).map(|entry| entry.clone())
    }

    /// Cache-aware variant used by high-frequency shell execution paths.
    pub async fn ensure_runtime_cached(session: &SessionMetadata) -> RuntimeResult<()> {
        wait_for_docker_provisioning(&session.id).await;
        prepare_docker_runtime(session, true, true, None)
            .await
            .map(|_| ())
    }

    pub fn try_mark_provisioning_in_flight(session_id: &str) -> bool {
        if DOCKER_PROVISIONING_IN_FLIGHT.contains_key(session_id) {
            return false;
        }
        DOCKER_PROVISIONING_IN_FLIGHT.insert(session_id.to_string(), ());
        DOCKER_PROVISIONING_WAITERS
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(Notify::new()));
        true
    }

    pub fn clear_provisioning_in_flight(session_id: &str) {
        DOCKER_PROVISIONING_IN_FLIGHT.remove(session_id);
        if let Some((_, notify)) = DOCKER_PROVISIONING_WAITERS.remove(session_id) {
            notify.notify_waiters();
        }
    }

    pub fn is_provisioning_in_flight(session_id: &str) -> bool {
        DOCKER_PROVISIONING_IN_FLIGHT.contains_key(session_id)
    }

    pub async fn provision_runtime_with_steps(
        session: &SessionMetadata,
        reporter: Option<DockerStepReporter>,
    ) -> RuntimeResult<()> {
        prepare_docker_runtime(session, false, false, reporter)
            .await
            .map(|_| ())
    }

    pub async fn create_docker_exec_command(
        session: &SessionMetadata,
        command: &str,
        env_vars: &std::collections::HashMap<String, String>,
    ) -> RuntimeResult<AsyncCommand> {
        let shell = prepare_docker_runtime(session, true, true, None).await?;

        let container_name = docker_container_name(session)?;
        let workdir = session_docker_workdir(session);
        let mut cmd = AsyncCommand::new("docker");
        apply_docker_cli_env(&mut cmd);
        cmd.args(["exec", "-i", "-w", &workdir]);

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

        cmd.arg(container_name);
        // runShell intentionally executes shell syntax; this mirrors the host shell path.
        cmd.args([shell.command(), "-lc", command]);
        Ok(cmd)
    }

    pub async fn spawn_docker_persistent_shell(
        session: &SessionMetadata,
    ) -> RuntimeResult<SpawnedShell> {
        let shell = prepare_docker_runtime(session, false, false, None).await?;

        let container_name = docker_container_name(session)?;
        let host_workspace = docker_host_workspace_path(session)?;
        let workdir = session_docker_workdir(session);
        let mut cmd = AsyncCommand::new("docker");
        apply_docker_cli_env(&mut cmd);
        cmd.args(["exec", "-i", "-w", &workdir]);

        if let Some(config) = &session.docker_config {
            for (key, value) in &config.env {
                validate_env_key(key).map_err(WorkspaceRuntimeError::InvalidConfig)?;
                validate_env_value(key, value).map_err(WorkspaceRuntimeError::InvalidConfig)?;
                cmd.arg("-e").arg(format!("{key}={value}"));
            }
        }

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
            initial_cwd: workdir,
            path_mapper: path_mapper_for_session(session, host_workspace),
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

        clear_session_docker_caches(&session.id);

        if !session_manage_lifecycle(session) {
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

// ── Helpers ──────────────────────────────────────────────────────────────────

fn clear_session_docker_caches(session_id: &str) {
    DOCKER_HEALTH_CACHE.remove(session_id);
    DOCKER_CONTAINER_READY_CACHE.remove(session_id);
    DOCKER_SHELL_CACHE.remove(session_id);
    DOCKER_ARCH_CACHE.remove(session_id);
    DOCKER_SESSION_LOCKS.remove(session_id);
    DOCKER_PROVISIONING_IN_FLIGHT.remove(session_id);
    DOCKER_PROVISIONING_WAITERS.remove(session_id);
}

async fn wait_for_docker_provisioning(session_id: &str) {
    let notify = DOCKER_PROVISIONING_WAITERS
        .get(session_id)
        .map(|entry| Arc::clone(entry.value()));
    if let Some(notify) = notify {
        if DOCKER_PROVISIONING_IN_FLIGHT.contains_key(session_id) {
            notify.notified().await;
        }
    }
}

async fn prepare_docker_runtime(
    session: &SessionMetadata,
    cache_health: bool,
    cache_container: bool,
    reporter: Option<DockerStepReporter>,
) -> RuntimeResult<ShellType> {
    if session.workspace_isolation != WorkspaceIsolationMode::Docker {
        return Err(WorkspaceRuntimeError::InvalidConfig(
            "prepare_docker_runtime called for non-Docker session".to_string(),
        ));
    }

    let lock = DOCKER_SESSION_LOCKS
        .entry(session.id.clone())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone();
    let _guard = lock.lock().await;

    let now = Instant::now();
    let ttl = std::time::Duration::from_secs(DOCKER_RUNTIME_CACHE_TTL_SECS);

    if cache_health {
        let needs_healthcheck = match DOCKER_HEALTH_CACHE.get(&session.id) {
            Some(entry) => now.duration_since(entry.0) >= ttl || !entry.1,
            None => true,
        };
        if needs_healthcheck {
            if let Some(reporter) = reporter.as_ref() {
                reporter("Checking Docker daemon");
            }
            WorkspaceRuntimeManager::healthcheck().await?;
            DOCKER_HEALTH_CACHE.insert(session.id.clone(), (now, true));
        }
    } else {
        if let Some(reporter) = reporter.as_ref() {
            reporter("Checking Docker daemon");
        }
        WorkspaceRuntimeManager::healthcheck().await?;
        DOCKER_HEALTH_CACHE.insert(session.id.clone(), (now, true));
    }

    if cache_container {
        if let (Some(ready_at), Some(shell)) = (
            DOCKER_CONTAINER_READY_CACHE.get(&session.id),
            DOCKER_SHELL_CACHE.get(&session.id),
        ) {
            if now.duration_since(*ready_at) < ttl {
                return Ok(*shell);
            }
        }
    }

    let container_name = docker_container_name(session)?;
    let host_workspace = docker_host_workspace_path(session)?;
    let image = session
        .docker_config
        .as_ref()
        .and_then(|config| config.image_ref())
        .unwrap_or("attached")
        .to_string();
    let shell = ensure_bash_image_contract(
        &session.id,
        &container_name,
        session,
        &host_workspace,
        &image,
        reporter.as_ref(),
    )
    .await?;

    let arch = match docker_container_architecture(&container_name).await {
        Ok(arch) => arch,
        Err(error) => {
            tracing::warn!(
                "Failed to inspect Docker architecture for {container_name}: {error}; falling back to host arch"
            );
            std::env::consts::ARCH.to_string()
        }
    };

    DOCKER_SHELL_CACHE.insert(session.id.clone(), shell);
    DOCKER_ARCH_CACHE.insert(session.id.clone(), arch);
    DOCKER_CONTAINER_READY_CACHE.insert(session.id.clone(), now);
    Ok(shell)
}

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

async fn docker_container_architecture(container_name: &str) -> RuntimeResult<String> {
    let arch = docker_output(["inspect", "-f", "{{.Architecture}}", container_name]).await?;
    Ok(normalize_docker_arch(&arch))
}

async fn ensure_bash_image_contract(
    session_id: &str,
    container_name: &str,
    session: &SessionMetadata,
    host_workspace: &Path,
    image: &str,
    reporter: Option<&DockerStepReporter>,
) -> RuntimeResult<ShellType> {
    if session_is_attach(session) {
        if !docker_container_exists(container_name).await? {
            return Err(WorkspaceRuntimeError::DockerCommandFailed(format!(
                "Attach container '{container_name}' was not found"
            )));
        }
        if !docker_container_running(container_name).await? {
            if let Some(reporter) = reporter {
                reporter("Starting attached container");
            }
            run_docker_status(["start", container_name]).await?;
        }
        if let Some(reporter) = reporter {
            reporter("Verifying shell");
        }
        return ensure_supported_shell(session_id, container_name).await;
    }

    if docker_container_exists(container_name).await? {
        verify_container_label(container_name, session_id).await?;
        if !docker_container_running(container_name).await? {
            if let Some(reporter) = reporter {
                reporter("Starting container");
            }
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
        let image_ref = config.image_ref().ok_or_else(|| {
            WorkspaceRuntimeError::InvalidConfig(
                "Managed Docker sessions require an image".to_string(),
            )
        })?;

        tokio::fs::create_dir_all(host_workspace)
            .await
            .map_err(|e| {
                WorkspaceRuntimeError::Io(format!(
                    "Failed to create Docker host workspace '{}': {e}",
                    host_workspace.display()
                ))
            })?;

        if let Some(reporter) = reporter {
            reporter(&format!("Pulling image {image}"));
        }

        let root_session_id = resolve_teamwork_root_session_id(session).await;
        let session_manager =
            crate::session::get_session_manager().map_err(WorkspaceRuntimeError::Io)?;
        let teamwork_dir = session_manager
            .get_directory_service()
            .get_teamwork_artifact_dir_unverified(&root_session_id);

        if session.org_id.is_some() || session.parent_session_id.is_some() {
            let _ = tokio::fs::create_dir_all(&teamwork_dir).await;
        }

        let tw_dir_arg = if teamwork_dir.exists() {
            Some(teamwork_dir.as_path())
        } else {
            None
        };

        let volume_args = build_docker_volume_args(host_workspace, tw_dir_arg)?;
        let label = format!("com.libragent.session_id={session_id}");
        let workdir = session_docker_workdir(session);
        let mut cmd = AsyncCommand::new("docker");
        apply_docker_cli_env(&mut cmd);
        cmd.args(["run", "-d", "--name", container_name, "--label", &label]);
        cmd.args(volume_args);
        cmd.args(["-w", &workdir]);

        if let Some(config) = &session.docker_config {
            append_port_binding_args(&mut cmd, config).await?;
        }

        if let Some(user) = current_uid_gid().await {
            cmd.args(["--user", &user]);
        }

        if let Some(reporter) = reporter {
            reporter("Starting container");
        }

        cmd.args([image_ref, "tail", "-f", "/dev/null"]);
        run_status_command(cmd).await?;
    }

    verify_container_label(container_name, session_id).await?;
    if let Some(reporter) = reporter {
        reporter("Verifying shell");
    }
    ensure_supported_shell(session_id, container_name).await
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

fn session_is_attach(session: &SessionMetadata) -> bool {
    session
        .docker_config
        .as_ref()
        .is_some_and(|config| config.is_attach())
}

fn session_manage_lifecycle(session: &SessionMetadata) -> bool {
    session
        .docker_config
        .as_ref()
        .map(|config| config.manage_lifecycle())
        .unwrap_or(true)
}

pub(crate) fn session_docker_workdir(session: &SessionMetadata) -> String {
    session
        .docker_config
        .as_ref()
        .map(|config| config.workdir().to_string())
        .unwrap_or_else(|| DEFAULT_DOCKER_WORKDIR.to_string())
}

fn path_mapper_for_session(session: &SessionMetadata, host_workspace: PathBuf) -> PathMappingLayer {
    PathMappingLayer::with_container_root(host_workspace, session_docker_workdir(session))
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

pub fn build_docker_volume_args(
    host_workspace: &Path,
    teamwork_dir: Option<&Path>,
) -> RuntimeResult<Vec<String>> {
    let mut args = Vec::new();
    let main_mount = format!("{}:/workspace", docker_mount_path(host_workspace)?);
    args.push("-v".to_string());
    args.push(main_mount);

    if let Some(tw_dir) = teamwork_dir {
        let tw_mount = format!(
            "{}:/workspace/.libragent/teamwork",
            docker_mount_path(tw_dir)?
        );
        args.push("-v".to_string());
        args.push(tw_mount);
    }

    Ok(args)
}

async fn resolve_teamwork_root_session_id(session: &SessionMetadata) -> String {
    if let Some(org_root_session_id) = &session.org_root_session_id {
        return org_root_session_id.clone();
    }

    if let Some(session_repo) = crate::state::try_get_session_repository() {
        let mut current = session.clone();
        for _ in 0..64 {
            let Some(parent_session_id) = current.parent_session_id.clone() else {
                return current.id;
            };

            match session_repo.get_session(&parent_session_id).await {
                Ok(Some(parent_session)) => {
                    current = parent_session;
                    if let Some(org_root_session_id) = &current.org_root_session_id {
                        return org_root_session_id.clone();
                    }
                }
                _ => return parent_session_id,
            }
        }
        current.id
    } else {
        session.id.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_build_docker_volume_args() {
        let host_workspace = Path::new("/home/user/workspace");

        // Test case 1: No teamwork directory
        let args = build_docker_volume_args(host_workspace, None).unwrap();
        assert!(args.contains(&"-v".to_string()));
        assert!(args
            .iter()
            .any(|arg| arg.contains("/home/user/workspace:/workspace")));

        // Test case 2: With teamwork directory
        let teamwork_dir = Path::new("/home/user/.libragent/teamwork-artifacts/123");
        let args = build_docker_volume_args(host_workspace, Some(teamwork_dir)).unwrap();
        assert_eq!(args.len(), 4);
        assert_eq!(args[0], "-v");
        assert!(args[1].contains("/home/user/workspace:/workspace"));
        assert_eq!(args[2], "-v");
        assert!(args[3].contains(
            "/home/user/.libragent/teamwork-artifacts/123:/workspace/.libragent/teamwork"
        ));
    }
}
