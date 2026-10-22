mod container;
mod docker_cli;
mod error;
mod session_config;

use std::collections::HashSet;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;

use crate::models::workspace_isolation::{
    validate_env_key, validate_env_value, WorkspaceIsolationMode,
};
use crate::repositories::SessionMetadata;
use crate::session_isolation::{ShellDialect, ShellType, SpawnedShell};
use dashmap::DashMap;
use once_cell::sync::Lazy;
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

use container::ensure_bash_image_contract;
use docker_cli::{
    apply_docker_cli_env, docker_container_architecture, docker_container_exists,
    docker_container_label, docker_output_slice, run_docker_status, verify_container_label,
};
use error::{as_docker_unavailable, RuntimeResult};
use session_config::{
    docker_container_name, docker_host_workspace_path, path_mapper_for_session,
    session_manage_lifecycle,
};

pub use docker_cli::normalize_docker_arch;
pub use error::{
    WorkspaceRuntimeError, AGENT_ERROR_DOCKER_NOT_AVAILABLE, AGENT_ERROR_DOCKER_NOT_INSTALLED,
};
pub use session_config::build_docker_volume_args;
pub(crate) use session_config::session_docker_workdir;

// ── Manager ──────────────────────────────────────────────────────────────────

pub struct WorkspaceRuntimeManager;

impl WorkspaceRuntimeManager {
    pub async fn healthcheck() -> RuntimeResult<()> {
        run_docker_status(["--version"])
            .await
            .map_err(as_docker_unavailable)?;
        run_docker_status(["info"])
            .await
            .map_err(as_docker_unavailable)?;
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
