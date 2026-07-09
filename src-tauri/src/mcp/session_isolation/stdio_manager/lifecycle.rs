use super::SessionMCPManager;
use crate::mcp::session_isolation::error::SessionMCPError;
use crate::mcp::session_isolation::process::MCPProcess;
use crate::mcp::types::{MCPServerConfig, TransportConfig};
use dashmap::DashMap;
use log::{debug, error, info, warn};
use rmcp::ServiceExt;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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

        let _guard = spawn_lock.lock().await;

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

        // 6. Spawn process with cross-platform command preparation
        // On Windows, this wraps .cmd/.bat files with cmd.exe
        let (final_command, final_args) =
            crate::mcp::utils::command_helper::prepare_command(command, args);

        let config_env_keys: Vec<&str> = env.keys().map(|k| k.as_str()).collect();
        info!(
            "Spawning MCP server '{}' for session '{}': raw={} {:?} -> final={} {:?} cwd={:?} config_env_keys={:?}",
            server_name,
            self.session_id,
            command,
            args,
            final_command,
            final_args,
            self.workspace_dir,
            config_env_keys
        );
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

        let transport =
            crate::mcp::session_isolation::channel_transport::spawn_channel_aware_stdio(
                cmd,
                server_name.to_string(),
                self.channel_event_tx.clone(),
            )
            .map_err(|e| {
                error!(
                    "Failed to spawn MCP server '{}' for session '{}': {} (final={} {:?})",
                    server_name, self.session_id, e, final_command, final_args
                );
                SessionMCPError::SpawnFailed(format!("{}", e))
            })?;

        debug!("Created transport for command: {} {:?}", command, args);

        // 7. Initialize rmcp client with timeout
        let timeout = Duration::from_secs(self.config.process_startup_timeout_seconds);
        let client = tokio::time::timeout(timeout, ().serve(transport))
            .await
            .map_err(|_| {
                error!(
                    "MCP server '{}' init timed out after {}s for session '{}' (final={} {:?}). Check preceding [MCP stderr:{}] lines.",
                    server_name,
                    self.config.process_startup_timeout_seconds,
                    self.session_id,
                    final_command,
                    final_args,
                    server_name
                );
                SessionMCPError::InitTimeout(server_name.to_string())
            })?
            .map_err(|e| {
                error!(
                    "MCP server '{}' init failed for session '{}': {} (final={} {:?}). Check preceding [MCP stderr:{}] lines.",
                    server_name, self.session_id, e, final_command, final_args, server_name
                );
                SessionMCPError::InitFailed(format!("{}", e))
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
