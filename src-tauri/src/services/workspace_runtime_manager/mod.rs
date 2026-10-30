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

use container::{ensure_bash_image_contract, stage_missing_skill_roots};
use docker_cli::{
    apply_docker_cli_env, docker_container_architecture, docker_container_exists,
    docker_container_label, docker_output_slice, docker_program, run_docker_status,
    verify_container_label,
};
use error::{as_docker_unavailable, RuntimeResult};

/// Passes `env` to `docker exec`. The wake URL is given as `-e KEY` with its value in the
/// docker CLI's own environment, so its token never appears in the host process list.
fn apply_exec_env(
    cmd: &mut AsyncCommand,
    env: impl IntoIterator<Item = (String, String)>,
) -> RuntimeResult<()> {
    for (key, value) in env {
        validate_env_key(&key).map_err(WorkspaceRuntimeError::InvalidConfig)?;
        validate_env_value(&key, &value).map_err(WorkspaceRuntimeError::InvalidConfig)?;
        if is_session_api_key_var(&key) {
            continue;
        }
        if key == crate::server::wake::WAKE_URL_ENV {
            cmd.arg("-e").arg(&key);
            cmd.env(&key, &value);
        } else {
            cmd.arg("-e").arg(format!("{key}={value}"));
        }
    }
    Ok(())
}

/// `docker exec` client for a stdio MCP server in a session container.
pub struct DockerMcpCommand {
    pub command: AsyncCommand,
    pub container: String,
    /// The CLI reads this file when the process starts; keep it for the process lifetime.
    pub env_file: Option<tempfile::TempPath>,
}

fn build_docker_mcp_command(
    session: &SessionMetadata,
    command: &str,
    args: &[String],
    env: &std::collections::HashMap<String, String>,
) -> RuntimeResult<DockerMcpCommand> {
    let container = docker_container_name(session)?;
    let mut cmd = AsyncCommand::new(docker_program());
    apply_docker_cli_env(&mut cmd);
    cmd.args(["exec", "-i", "-w", &session_docker_workdir(session)]);

    let mut merged_env = session
        .docker_config
        .as_ref()
        .map(|config| config.env.clone())
        .unwrap_or_default();
    merged_env.extend(env.iter().map(|(k, v)| (k.clone(), v.clone())));
    let env_file = write_exec_env_file(merged_env)?;
    if let Some(path) = &env_file {
        cmd.arg("--env-file").arg(path.as_os_str());
    }

    cmd.arg(&container).arg(command).args(args);
    Ok(DockerMcpCommand {
        command: cmd,
        container,
        env_file,
    })
}

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

/// Builds `docker exec -i [-u 0] -w <workdir> [env] <container> <shell> -lc <command>`.
fn build_docker_exec_command(
    session: &SessionMetadata,
    shell: ShellType,
    command: &str,
    env_vars: &std::collections::HashMap<String, String>,
    cwd_override: Option<&str>,
    run_as_root: bool,
) -> RuntimeResult<AsyncCommand> {
    let container_name = docker_container_name(session)?;
    let workdir = cwd_override
        .map(str::to_string)
        .unwrap_or_else(|| session_docker_workdir(session));
    let mut cmd = AsyncCommand::new("docker");
    apply_docker_cli_env(&mut cmd);
    cmd.args(["exec", "-i"]);
    if run_as_root {
        // The container itself runs as the host user; only this exec gets uid 0.
        cmd.args(["-u", "0"]);
    }
    cmd.args(["-w", &workdir]);

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

    apply_exec_env(&mut cmd, merged_env)?;

    cmd.arg(container_name);
    // runShell intentionally executes shell syntax; this mirrors the host shell path.
    cmd.args([shell.command(), "-lc", command]);
    Ok(cmd)
}

/// Writes the server env to a private (0600) temp file for `docker exec --env-file`. Values
/// stay out of the host process list, and out of the docker CLI's own environment: there,
/// `HOME`, `DOCKER_HOST`, `DOCKER_CONTEXT` or `DOCKER_CONFIG` would change which daemon the
/// CLI uses.
fn write_exec_env_file(
    env: std::collections::HashMap<String, String>,
) -> RuntimeResult<Option<tempfile::TempPath>> {
    use std::io::Write;

    let mut entries = Vec::with_capacity(env.len());
    for (key, value) in env {
        validate_env_key(&key).map_err(WorkspaceRuntimeError::InvalidConfig)?;
        validate_env_value(&key, &value).map_err(WorkspaceRuntimeError::InvalidConfig)?;
        if value.contains(['\n', '\r']) {
            return Err(WorkspaceRuntimeError::InvalidConfig(format!(
                "Env value of '{key}' contains a line break; stdio MCP servers in a Docker \
                 session cannot receive it"
            )));
        }
        if !is_session_api_key_var(&key) {
            entries.push((key, value));
        }
    }
    if entries.is_empty() {
        return Ok(None);
    }
    entries.sort();

    let io_error =
        |e: std::io::Error| WorkspaceRuntimeError::Io(format!("Failed to write MCP env file: {e}"));
    let mut file = tempfile::Builder::new()
        .prefix("libragent-mcp-env-")
        .tempfile()
        .map_err(io_error)?;
    for (key, value) in &entries {
        writeln!(file, "{key}={value}").map_err(io_error)?;
    }
    file.flush().map_err(io_error)?;
    Ok(Some(file.into_temp_path()))
}

/// The Session API key must never reach a container, whatever the caller passes.
fn is_session_api_key_var(key: &str) -> bool {
    key == crate::server::api_key::ENV_VAR || key == crate::server::api_key::ENV_FILE_VAR
}
use session_config::{
    docker_container_name, docker_host_workspace_path, path_mapper_for_session,
    session_manage_lifecycle,
};

pub use docker_cli::{docker_bridge_gateway, normalize_docker_arch};
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
        build_docker_exec_command(session, shell, command, env_vars, cwd_override, run_as_root)
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

#[cfg(test)]
mod exec_env_tests {
    use super::*;

    fn args_and_env(env: Vec<(&str, &str)>) -> (Vec<String>, Vec<(String, Option<String>)>) {
        let mut cmd = AsyncCommand::new("docker");
        apply_exec_env(
            &mut cmd,
            env.into_iter().map(|(k, v)| (k.to_string(), v.to_string())),
        )
        .unwrap();
        let std_cmd = cmd.as_std();
        let args = std_cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let envs = std_cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        (args, envs)
    }

    #[test]
    fn wake_url_value_stays_out_of_the_docker_arguments() {
        let url = "http://host.docker.internal:4000/api/sessions/s/wake/secrettoken";
        let (args, envs) = args_and_env(vec![("LIBRAGENT_WAKE_URL", url), ("PLAIN", "1")]);
        assert!(args.iter().all(|arg| !arg.contains("secrettoken")));
        assert!(args.contains(&"LIBRAGENT_WAKE_URL".to_string()));
        assert!(args.contains(&"PLAIN=1".to_string()));
        assert!(envs.contains(&("LIBRAGENT_WAKE_URL".to_string(), Some(url.to_string()))));
    }

    fn docker_session(env: Vec<(&str, &str)>) -> SessionMetadata {
        SessionMetadata {
            id: "s1".to_string(),
            name: None,
            status: crate::repositories::SessionStatus::Idle,
            model: "m".to_string(),
            provider: "p".to_string(),
            assistant_id: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            max_depth: None,
            max_fanout: None,
            org_id: None,
            org_name: None,
            org_root_session_id: None,
            created_at: 0,
            updated_at: 0,
            last_viewed_at: None,
            last_message_at: None,
            last_attention_at: None,
            last_attention_reason: None,
            is_bookmarked: false,
            execution_mode: crate::execution_mode::ExecutionMode::Normal,
            workspace_override: None,
            workspace_isolation: WorkspaceIsolationMode::Docker,
            docker_config: Some(crate::models::workspace_isolation::DockerWorkspaceConfig {
                image: Some("python:3.12-slim".to_string()),
                attach_container: None,
                workdir: Some("/work".to_string()),
                manage_lifecycle: None,
                env: env
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                port_bindings: Vec::new(),
            }),
            docker_container_name: Some("libragent-session-s1".to_string()),
            docker_host_workspace_path: Some("/tmp/s1".to_string()),
        }
    }

    fn command_parts(cmd: &AsyncCommand) -> (Vec<String>, Vec<(String, Option<String>)>) {
        let std_cmd = cmd.as_std();
        let args = std_cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let envs = std_cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        (args, envs)
    }

    #[test]
    fn mcp_command_execs_argv_in_the_session_container() {
        let session = docker_session(vec![]);
        let built = build_docker_mcp_command(
            &session,
            "npx",
            &["-y".to_string(), "@scope/server".to_string()],
            &std::collections::HashMap::new(),
        )
        .unwrap();
        assert_eq!(built.container, "libragent-session-s1");
        assert!(built.env_file.is_none());
        let (args, _) = command_parts(&built.command);
        assert_eq!(
            args,
            vec![
                "exec",
                "-i",
                "-w",
                "/work",
                "libragent-session-s1",
                "npx",
                "-y",
                "@scope/server"
            ]
        );
    }

    #[test]
    fn mcp_command_passes_env_values_in_a_private_file() {
        let session = docker_session(vec![("SESSION_VAR", "from-session")]);
        let env = std::collections::HashMap::from([
            ("API_TOKEN".to_string(), "secret-value".to_string()),
            ("SESSION_VAR".to_string(), "from-server".to_string()),
            (
                "LIBRAGENT_SESSION_API_KEY".to_string(),
                "libr_key".to_string(),
            ),
        ]);
        let built = build_docker_mcp_command(&session, "node", &[], &env).unwrap();
        let (args, envs) = command_parts(&built.command);
        let env_file = built.env_file.as_ref().expect("env file");
        let path = env_file.to_string_lossy().into_owned();

        assert_eq!(
            args,
            vec![
                "exec",
                "-i",
                "-w",
                "/work",
                "--env-file",
                path.as_str(),
                "libragent-session-s1",
                "node"
            ]
        );
        assert_eq!(
            std::fs::read_to_string(env_file).unwrap(),
            "API_TOKEN=secret-value\nSESSION_VAR=from-server\n"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(env_file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert!(envs
            .iter()
            .all(|(_, value)| value.as_deref() != Some("secret-value")));

        let file_path = env_file.to_path_buf();
        drop(built);
        assert!(!file_path.exists(), "env file is removed with the command");
    }

    #[test]
    fn server_env_does_not_change_how_the_docker_cli_connects() {
        let session = docker_session(vec![]);
        let env = std::collections::HashMap::from([
            ("HOME".to_string(), "/container/home".to_string()),
            (
                "DOCKER_HOST".to_string(),
                "tcp://elsewhere:2375".to_string(),
            ),
            ("DOCKER_CONTEXT".to_string(), "other".to_string()),
            (
                "DOCKER_CONFIG".to_string(),
                "/container/.docker".to_string(),
            ),
            ("PATH".to_string(), "/opt/node/bin".to_string()),
        ]);
        let built = build_docker_mcp_command(&session, "node", &[], &env).unwrap();
        let (_, envs) = command_parts(&built.command);
        for (key, value) in &env {
            assert!(
                !envs.contains(&(key.clone(), Some(value.clone()))),
                "server value of {key} reached the docker CLI env"
            );
        }
        let file = std::fs::read_to_string(built.env_file.as_ref().unwrap()).unwrap();
        assert!(file.contains("DOCKER_HOST=tcp://elsewhere:2375\n"));
        assert!(file.contains("HOME=/container/home\n"));
    }

    #[test]
    fn env_value_with_a_line_break_is_rejected() {
        let session = docker_session(vec![]);
        let env = std::collections::HashMap::from([("A".to_string(), "x\nB=y".to_string())]);
        assert!(build_docker_mcp_command(&session, "node", &[], &env).is_err());
    }

    #[test]
    fn mcp_command_does_not_forward_host_env_into_the_container() {
        let session = docker_session(vec![]);
        let built =
            build_docker_mcp_command(&session, "node", &[], &std::collections::HashMap::new())
                .unwrap();
        let (args, envs) = command_parts(&built.command);
        assert!(!args.iter().any(|arg| arg == "-e" || arg == "--env-file"));
        // The docker CLI itself still gets the isolated host env (PATH, HOME, …).
        assert!(envs.iter().any(|(key, _)| key == "PATH"));
    }

    #[test]
    fn shell_command_runs_as_root_only_when_asked() {
        let session = docker_session(vec![]);
        let env = std::collections::HashMap::new();

        let root = build_docker_exec_command(
            &session,
            ShellType::Bash,
            "apt-get update",
            &env,
            None,
            true,
        )
        .unwrap();
        let (args, _) = command_parts(&root);
        assert_eq!(
            args,
            vec![
                "exec",
                "-i",
                "-u",
                "0",
                "-w",
                "/work",
                "libragent-session-s1",
                "bash",
                "-lc",
                "apt-get update"
            ]
        );

        let user =
            build_docker_exec_command(&session, ShellType::Bash, "id", &env, None, false).unwrap();
        let (args, _) = command_parts(&user);
        assert!(!args.iter().any(|arg| arg == "-u"));
    }

    #[test]
    fn root_is_available_only_in_a_managed_session_container() {
        let managed = docker_session(vec![]);
        assert_eq!(WorkspaceRuntimeManager::root_exec_blocker(&managed), None);

        let mut attached = docker_session(vec![]);
        if let Some(config) = attached.docker_config.as_mut() {
            config.attach_container = Some("dev-box".to_string());
        }
        assert_eq!(
            WorkspaceRuntimeManager::root_exec_blocker(&attached),
            Some(RootExecBlocker::AttachedContainer)
        );

        let mut host = docker_session(vec![]);
        host.workspace_isolation = WorkspaceIsolationMode::Host;
        assert_eq!(
            WorkspaceRuntimeManager::root_exec_blocker(&host),
            Some(RootExecBlocker::HostSession)
        );
    }

    #[test]
    fn session_api_key_variables_are_never_passed() {
        let (args, envs) = args_and_env(vec![
            ("LIBRAGENT_SESSION_API_KEY", "libr_secret"),
            ("LIBRAGENT_SESSION_API_KEY_FILE", "/tmp/key"),
        ]);
        assert!(args.is_empty());
        assert!(envs.is_empty());
    }
}
