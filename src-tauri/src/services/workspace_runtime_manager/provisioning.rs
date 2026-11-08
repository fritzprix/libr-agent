//! Per-session Docker runtime state (locks, health/readiness/shell/arch caches with a TTL,
//! provisioning waiters) and `prepare_docker_runtime`, which fills it.

use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use tokio::sync::{Mutex, Notify};

use crate::models::workspace_isolation::WorkspaceIsolationMode;
use crate::repositories::SessionMetadata;
use crate::session_isolation::ShellType;

use super::container::{ensure_bash_image_contract, stage_missing_skill_roots};
use super::docker_cli::docker_container_architecture;
use super::error::RuntimeResult;
use super::session_config::{docker_container_name, docker_host_workspace_path};
use super::{DockerStepReporter, WorkspaceRuntimeError, WorkspaceRuntimeManager};

/// Per-session mutex to serialize container creation and health-check attempts.
static DOCKER_SESSION_LOCKS: Lazy<DashMap<String, Arc<Mutex<()>>>> = Lazy::new(DashMap::new);

/// Health-check cache: session_id → (last_check_time, is_healthy).
/// Avoids running `docker --version` + `docker info` on every shell command.
static DOCKER_HEALTH_CACHE: Lazy<DashMap<String, (Instant, bool)>> = Lazy::new(DashMap::new);

/// Container readiness cache: session_id → last verified time.
static DOCKER_CONTAINER_READY_CACHE: Lazy<DashMap<String, Instant>> = Lazy::new(DashMap::new);

/// Resolved shell type cache: session_id → bash/sh.
pub(super) static DOCKER_SHELL_CACHE: Lazy<DashMap<String, ShellType>> = Lazy::new(DashMap::new);

/// Resolved container architecture cache: session_id → Rust-style arch (x86_64/aarch64/…).
pub(super) static DOCKER_ARCH_CACHE: Lazy<DashMap<String, String>> = Lazy::new(DashMap::new);

/// Waiters notified when background Docker provisioning completes for a session.
pub(super) static DOCKER_PROVISIONING_WAITERS: Lazy<DashMap<String, Arc<Notify>>> =
    Lazy::new(DashMap::new);

/// Tracks sessions with an in-flight provisioning task to avoid duplicate spawns.
pub(super) static DOCKER_PROVISIONING_IN_FLIGHT: Lazy<DashMap<String, ()>> =
    Lazy::new(DashMap::new);

const DOCKER_RUNTIME_CACHE_TTL_SECS: u64 = 30;

pub(super) fn clear_session_docker_caches(session_id: &str) {
    DOCKER_HEALTH_CACHE.remove(session_id);
    DOCKER_CONTAINER_READY_CACHE.remove(session_id);
    DOCKER_SHELL_CACHE.remove(session_id);
    DOCKER_ARCH_CACHE.remove(session_id);
    DOCKER_SESSION_LOCKS.remove(session_id);
    DOCKER_PROVISIONING_IN_FLIGHT.remove(session_id);
    DOCKER_PROVISIONING_WAITERS.remove(session_id);
}

pub(super) async fn wait_for_docker_provisioning(session_id: &str) {
    let notify = DOCKER_PROVISIONING_WAITERS
        .get(session_id)
        .map(|entry| Arc::clone(entry.value()));
    if let Some(notify) = notify {
        if DOCKER_PROVISIONING_IN_FLIGHT.contains_key(session_id) {
            notify.notified().await;
        }
    }
}

pub(super) async fn prepare_docker_runtime(
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
    stage_missing_skill_roots(&container_name).await;

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
