mod command;
mod container;
mod docker_cli;
mod error;
mod provisioning;
mod root_exec;
mod session_config;

#[cfg(test)]
mod exec_env_tests;

use std::collections::{BTreeMap, HashSet};
use std::process::Stdio;
use std::sync::Arc;

use crate::models::workspace_isolation::{
    validate_env_key, validate_env_value, WorkspaceIsolationMode,
};
use crate::repositories::SessionMetadata;
use crate::session_isolation::{ShellDialect, ShellType, SpawnedShell};
use tokio::process::Command as AsyncCommand;
use tokio::sync::Notify;

pub type DockerStepReporter = Arc<dyn Fn(&str) + Send + Sync>;

use command::{apply_secret_exec_env, build_docker_exec_command, build_docker_mcp_command};
use docker_cli::{
    apply_docker_cli_env, docker_container_exists, docker_container_label, docker_output_slice,
    run_docker_status, verify_container_label,
};
use error::{as_docker_unavailable, RuntimeResult};
use provisioning::{
    clear_session_docker_caches, prepare_docker_runtime, wait_for_docker_provisioning,
    DOCKER_ARCH_CACHE, DOCKER_PROVISIONING_IN_FLIGHT, DOCKER_PROVISIONING_WAITERS,
    DOCKER_SHELL_CACHE,
};

pub use command::DockerMcpCommand;

/// Reason that root execution is not available for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootExecBlocker {
    /// The session runs commands on the host.
    HostSession,
    /// The session uses a container that LibrAgent did not create.
    AttachedContainer,
}

impl RootExecBlocker {
    pub fn message(self) -> &'static str {
        match self {
            Self::HostSession => {
                "asRoot is only available in a Docker session that uses a LibrAgent-managed container"
            }
            Self::AttachedContainer => {
                "asRoot is not available for an attached container: LibrAgent does not manage it"
            }
        }
    }
}

use session_config::{
    docker_container_name, docker_host_workspace_path, path_mapper_for_session,
    session_manage_lifecycle,
};

pub use docker_cli::{docker_bridge_gateway, normalize_docker_arch};
pub use error::{
    WorkspaceRuntimeError, AGENT_ERROR_DOCKER_NOT_AVAILABLE, AGENT_ERROR_DOCKER_NOT_INSTALLED,
};
pub use root_exec::{as_root_requested, stopped_processes_note, RootExecLease};
pub use session_config::build_docker_volume_args;
pub(crate) use session_config::{refuse_home_workspace, session_docker_workdir};

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
        secrets: &BTreeMap<String, String>,
        cwd_override: Option<&str>,
        run_as_root: bool,
    ) -> RuntimeResult<AsyncCommand> {
        if run_as_root {
            if let Some(blocker) = Self::root_exec_blocker(session) {
                return Err(WorkspaceRuntimeError::InvalidConfig(
                    blocker.message().to_string(),
                ));
            }
        }
        let shell = prepare_docker_runtime(session, true, true, None).await?;
        if run_as_root {
            root_exec::verify_root_exec_ready(session).await?;
        }
        build_docker_exec_command(
            session,
            shell,
            command,
            env_vars,
            secrets,
            cwd_override,
            run_as_root,
        )
    }

    /// Why a shell command of this session cannot run as root, or `None` when it can.
    /// Only a container that LibrAgent created and manages allows root.
    pub fn root_exec_blocker(session: &SessionMetadata) -> Option<RootExecBlocker> {
        if session.workspace_isolation != WorkspaceIsolationMode::Docker {
            return Some(RootExecBlocker::HostSession);
        }
        if session
            .docker_config
            .as_ref()
            .is_some_and(|config| config.is_attach())
        {
            return Some(RootExecBlocker::AttachedContainer);
        }
        None
    }

    /// Builds `docker exec -i -w <workdir> <container> <command> <args...>` for a stdio MCP
    /// server of a Docker session. Argv goes unchanged (no shell). Only the session Docker env
    /// and the server env reach the container, never the host env.
    pub async fn create_docker_mcp_command(
        session: &SessionMetadata,
        command: &str,
        args: &[String],
        env: &std::collections::HashMap<String, String>,
    ) -> RuntimeResult<DockerMcpCommand> {
        Self::prepare_mcp_container(session).await?;
        build_docker_mcp_command(session, command, args, env)
    }

    /// Waits for provisioning and makes sure the session container runs (may pull an image).
    pub async fn prepare_mcp_container(session: &SessionMetadata) -> RuntimeResult<()> {
        wait_for_docker_provisioning(&session.id).await;
        prepare_docker_runtime(session, true, true, None)
            .await
            .map(|_| ())
    }

    /// `secrets` are passed as `-e KEY` with the value in the docker CLI env, so no value
    /// appears in the host process list.
    pub async fn spawn_docker_persistent_shell(
        session: &SessionMetadata,
        secrets: &BTreeMap<String, String>,
    ) -> RuntimeResult<SpawnedShell> {
        let shell = prepare_docker_runtime(session, false, false, None).await?;

        let container_name = docker_container_name(session)?;
        let host_workspace = docker_host_workspace_path(session)?;
        let workdir = session_docker_workdir(session);
        let mut cmd = AsyncCommand::new("docker");
        apply_docker_cli_env(&mut cmd);
        cmd.args(["exec", "-i", "-w", &workdir]);

        apply_secret_exec_env(&mut cmd, secrets)?;

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
