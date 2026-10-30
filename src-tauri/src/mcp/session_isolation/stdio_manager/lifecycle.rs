use super::SessionMCPManager;
use crate::mcp::session_isolation::channel_transport::ContainerExec;
use crate::mcp::session_isolation::error::SessionMCPError;
use crate::mcp::session_isolation::exec_target::{
    is_runtime_missing, runtime_missing_message, session_image, StdioExecTarget,
};
use crate::mcp::session_isolation::process::MCPProcess;
use crate::mcp::types::{MCPServerConfig, TransportConfig};
use crate::services::workspace_runtime_manager::WorkspaceRuntimeError;
use crate::services::WorkspaceRuntimeManager;
use dashmap::DashMap;
use log::{debug, error, info, warn};
use rmcp::ServiceExt;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Only failures that repeat until the user changes the session become `ContainerUnavailable`
/// (no automatic retry). Daemon and network failures stay `SpawnFailed` (retried with backoff).
fn container_prepare_error(error: WorkspaceRuntimeError) -> SessionMCPError {
    if error.is_permanent() {
        SessionMCPError::ContainerUnavailable(error.to_string())
    } else {
        SessionMCPError::SpawnFailed(error.to_string())
    }
}

#[cfg(test)]
mod container_prepare_error_tests {
    use super::*;

    #[test]
    fn only_a_missing_image_or_attach_container_is_unavailable() {
        for permanent in [
            WorkspaceRuntimeError::ImageNotFound("Cannot pull Docker image 'x'".to_string()),
            WorkspaceRuntimeError::AttachContainerNotFound("c".to_string()),
        ] {
            assert!(matches!(
                container_prepare_error(permanent),
                SessionMCPError::ContainerUnavailable(_)
            ));
        }
        for transient in [
            WorkspaceRuntimeError::DockerNotAvailable("daemon down".to_string()),
            WorkspaceRuntimeError::DockerCommandFailed("dial tcp: i/o timeout".to_string()),
            WorkspaceRuntimeError::Io("disk full".to_string()),
        ] {
            assert!(matches!(
                container_prepare_error(transient),
                SessionMCPError::SpawnFailed(_)
            ));
        }
    }
}

impl SessionMCPManager {
    /// Creates a new SessionMCPManager for the given session.
    pub fn new(
        session_id: String,
        server_configs: HashMap<String, MCPServerConfig>,
        config: crate::mcp::session_isolation_config::SessionIsolationConfig,
        workspace_dir: std::path::PathBuf,
        channel_event_tx: crate::mcp::session_isolation::channel_events::ChannelEventSender,
    ) -> Self {
        let idle_timeout = Duration::from_secs(config.idle_timeout_minutes * 60);

        Self {
            session_id,
            active_processes: Arc::new(RwLock::new(HashMap::new())),
            last_activity: Arc::new(RwLock::new(HashMap::new())),
            spawn_locks: Arc::new(DashMap::new()),
            idle_timeout,
            server_configs,
            config,
            active_call_tokens: Arc::new(RwLock::new(HashMap::new())),
            channel_metadata: Arc::new(RwLock::new(HashMap::new())),
            workspace_dir,
            channel_event_tx,
            shutdown_token: CancellationToken::new(),
        }
    }

    /// Ensures the specified MCP server process is running for this session.
    ///
    /// This is race-safe: if multiple tasks try to spawn the same server at the
    /// same time, only one will actually start the process. The other tasks will
    /// wait on the per-server spawn lock, re-check the process table after the
    /// lock is released, and then reuse the already-running process.
    ///
    /// Call this before executing any MCP request that depends on the given
    /// `server_name`; it is cheap when the server is already running due to the
    /// fast-path check on `active_processes`.
    pub(crate) async fn ensure_process_running(
        &self,
        server_name: &str,
    ) -> Result<(), SessionMCPError> {
        self.ensure_process_running_with_init_timeout(
            server_name,
            Duration::from_secs(self.config.process_startup_timeout_seconds),
        )
        .await
    }

    /// Same as [`Self::ensure_process_running`] with an explicit initialize timeout.
    ///
    /// Only background discovery passes a longer one (the server startup cap): there the
    /// session has already stopped waiting, so a slow server may keep loading. Tool calls
    /// keep the configured startup timeout.
    pub(crate) async fn ensure_process_running_with_init_timeout(
        &self,
        server_name: &str,
        init_timeout: Duration,
    ) -> Result<(), SessionMCPError> {
        // 1. Fast path: check if already running
        {
            let processes = self.active_processes.read().await;
            if processes.contains_key(server_name) {
                return Ok(());
            }
        }

        // 2. Acquire spawn lock for this server
        let spawn_lock = self
            .spawn_locks
            .entry(server_name.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();

        // Bounded by the same timeout as the initialize: while background discovery holds the
        // lock for a slow server, a tool call must not wait longer than its configured
        // startup timeout.
        let _guard = tokio::time::timeout(init_timeout, spawn_lock.lock())
            .await
            .map_err(|_| {
                warn!(
                    "MCP server '{}' is still starting for session '{}'; gave up waiting for it after {}s",
                    server_name,
                    self.session_id,
                    init_timeout.as_secs()
                );
                SessionMCPError::InitTimeout(format!(
                    "{} is still starting; try again shortly",
                    server_name
                ))
            })?;

        if self.shutdown_token.is_cancelled() {
            return Err(SessionMCPError::SessionClosed);
        }

        // 3. Double-check after acquiring lock (another task may have spawned)
        {
            let processes = self.active_processes.read().await;
            if processes.contains_key(server_name) {
                return Ok(());
            }
        }

        // 4. Get server config
        let config = self
            .server_configs
            .get(server_name)
            .ok_or_else(|| SessionMCPError::ServerNotFound(server_name.to_string()))?;

        // 5. Extract stdio config
        let (command, args, env) = match &config.transport {
            TransportConfig::Stdio { command, args, env } => (command, args, env),
            _ => return Err(SessionMCPError::InvalidTransport("Expected stdio".into())),
        };

        // 6. Build the command where this session runs stdio servers
        let target = StdioExecTarget::for_session(Some(&self.session_id))
            .await
            .map_err(SessionMCPError::SpawnFailed)?;
        let container_session = target.container_session();
        let (cmd, container) = match container_session {
            Some(session) => {
                let config_env_keys: Vec<&str> = env.keys().map(|k| k.as_str()).collect();
                info!(
                    "Spawning MCP server '{}' for session '{}' in its container: {} config_env_keys={:?}",
                    server_name, self.session_id, command, config_env_keys
                );
                let docker =
                    WorkspaceRuntimeManager::create_docker_mcp_command(session, command, args, env)
                        .await
                        .map_err(container_prepare_error)?;
                let exec = ContainerExec {
                    container: docker.container,
                    env_file: docker.env_file,
                };
                (docker.command, Some(exec))
            }
            None => (self.host_command(server_name, command, args, env)?, None),
        };
        let spawn_desc = match &container {
            Some(exec) => format!("{command} in container '{}'", exec.container),
            None => format!("{command} on host"),
        };
        debug!("MCP server '{}' args: {:?}", server_name, args);

        let transport =
            crate::mcp::session_isolation::channel_transport::spawn_channel_aware_stdio(
                cmd,
                server_name.to_string(),
                self.channel_event_tx.clone(),
                container,
            )
            .map_err(|e| {
                error!(
                    "Failed to spawn MCP server '{}' for session '{}': {} ({})",
                    server_name, self.session_id, e, spawn_desc
                );
                SessionMCPError::SpawnFailed(format!("{}", e))
            })?;
        let exit_report = transport.exit_report();

        debug!("Created transport for command: {}", spawn_desc);

        // 7. Initialize rmcp client with timeout
        // A dead process makes `serve` return an error right away; the timeout only catches
        // one that never answers. Session shutdown drops the pending initialize (and with it
        // the child process).
        let init_timeout_seconds = init_timeout.as_secs();
        let serve_result = tokio::select! {
            _ = self.shutdown_token.cancelled() => {
                return Err(SessionMCPError::SessionClosed);
            }
            result = tokio::time::timeout(init_timeout, ().serve(transport)) => result,
        };
        let client = serve_result
            .map_err(|_| {
                error!(
                    "MCP server '{}' init timed out after {}s for session '{}' ({}). Check preceding [MCP stderr:{}] lines.",
                    server_name,
                    init_timeout_seconds,
                    self.session_id,
                    spawn_desc,
                    server_name
                );
                SessionMCPError::InitTimeout(server_name.to_string())
            })?
            .map_err(|e| {
                error!(
                    "MCP server '{}' init failed for session '{}': {} ({}). Check preceding [MCP stderr:{}] lines.",
                    server_name, self.session_id, e, spawn_desc, server_name
                );
                match container_session {
                    Some(session) if is_runtime_missing(&exit_report) => {
                        SessionMCPError::RuntimeMissing(runtime_missing_message(
                            command,
                            &session_image(session),
                        ))
                    }
                    _ => SessionMCPError::InitFailed(format!("{}", e)),
                }
            })?;

        self.update_channel_metadata(server_name, client.peer_info())
            .await;

        info!("Successfully connected to MCP server: {}", server_name);

        // 8. Store process and update activity timestamp
        let process = MCPProcess::new(client);

        let mut processes = self.active_processes.write().await;
        processes.insert(server_name.to_string(), process);

        let mut activity = self.last_activity.write().await;
        activity.insert(server_name.to_string(), Instant::now());

        Ok(())
    }

    /// Host spawn: Windows `.cmd` wrapping, host PATH diagnostics, isolated host env.
    pub(super) fn host_command(
        &self,
        server_name: &str,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Command, SessionMCPError> {
        // On Windows, this wraps .cmd/.bat files with cmd.exe
        let (final_command, final_args) =
            crate::mcp::utils::command_helper::prepare_command(command, args);

        let config_env_keys: Vec<&str> = env.keys().map(|k| k.as_str()).collect();
        info!(
            "Spawning MCP server '{}' for session '{}': raw={} -> final={} cwd={:?} config_env_keys={:?}",
            server_name,
            self.session_id,
            command,
            final_command,
            self.workspace_dir,
            config_env_keys
        );
        debug!("MCP server '{}' final args: {:?}", server_name, final_args);
        log_spawn_path_diagnostics(server_name, command);

        let mut cmd = Command::new(&final_command);
        for arg in &final_args {
            cmd.arg(arg);
        }

        if !self.workspace_dir.exists() {
            std::fs::create_dir_all(&self.workspace_dir).map_err(|e| {
                SessionMCPError::SpawnFailed(format!(
                    "Failed to create session workspace '{}': {}",
                    self.workspace_dir.display(),
                    e
                ))
            })?;
        }

        cmd.current_dir(&self.workspace_dir);

        // Apply environment isolation:
        // 1. Clear all inherited environment variables to prevent secret leakage
        cmd.env_clear();

        // 2. Re-apply whitelisted system variables
        for (k, v) in crate::utils::env::get_isolated_env() {
            cmd.env(k, v);
        }

        // 3. Apply user-defined variables from config (can override system vars)
        for (key, value) in env {
            cmd.env(key, value);
        }

        // Hide the console window for GUI launches. Combined with inherited
        // stderr this historically broke Node/`npx` MCP initialize on Windows;
        // spawn_channel_aware_stdio() MUST keep stderr piped/null (see
        // configure_mcp_child_stdio / MCP_STDIO_STDERR_MUST_NOT_INHERIT).
        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        Ok(cmd)
    }
}

/// Log PATH / binary-resolution diagnostics for MCP stdio spawn failures.
///
/// Keeps secrets out of logs: only PATH entry count, whether common Node/Python
/// tool directories appear, and whether the configured command resolves.
fn log_spawn_path_diagnostics(server_name: &str, command: &str) {
    let effective_path = crate::utils::env::get_effective_path();
    let path_dirs: Vec<PathBuf> = std::env::split_paths(&effective_path).collect();
    let path_len = path_dirs.len();
    let markers = [
        "nodejs",
        "pi-node",
        "npm",
        "pnpm",
        "Python",
        "Scripts",
        ".local\\bin",
        ".local/bin",
        ".cargo\\bin",
        ".cargo/bin",
    ];
    let matched_markers: Vec<&str> = markers
        .iter()
        .copied()
        .filter(|marker| {
            let needle = marker.to_ascii_lowercase();
            path_dirs
                .iter()
                .any(|dir| dir.to_string_lossy().to_ascii_lowercase().contains(&needle))
        })
        .collect();

    match resolve_command_on_path(command, &path_dirs) {
        Some(path) => info!(
            "MCP spawn diagnostics server='{}' command='{}' resolved='{}' path_entries={} markers={:?}",
            server_name,
            command,
            path.display(),
            path_len,
            matched_markers
        ),
        None => warn!(
            "MCP spawn diagnostics server='{}' command='{}' resolved=NOT_FOUND path_entries={} markers={:?}. Isolated PATH may be missing Node/Python shims.",
            server_name, command, path_len, matched_markers
        ),
    }
}

fn resolve_command_on_path(command: &str, path_dirs: &[PathBuf]) -> Option<PathBuf> {
    let command_path = Path::new(command);
    if command_path.is_absolute() || command_path.components().count() > 1 {
        return command_path.exists().then(|| command_path.to_path_buf());
    }

    #[cfg(windows)]
    let candidates: Vec<String> = {
        let pathext =
            std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        let mut names = vec![command.to_string()];
        for ext in pathext.split(';').filter(|e| !e.is_empty()) {
            let ext = if ext.starts_with('.') {
                ext.to_string()
            } else {
                format!(".{}", ext)
            };
            if !command
                .to_ascii_lowercase()
                .ends_with(&ext.to_ascii_lowercase())
            {
                names.push(format!("{}{}", command, ext));
            }
        }
        names
    };

    #[cfg(not(windows))]
    let candidates: Vec<String> = vec![command.to_string()];

    for dir in path_dirs {
        for name in &candidates {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}
