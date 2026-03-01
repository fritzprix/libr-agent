use super::SessionMCPManager;
use crate::mcp::session_isolation::error::SessionMCPError;
use crate::mcp::session_isolation::process::MCPProcess;
use crate::mcp::types::{MCPServerConfig, TransportConfig};
use dashmap::DashMap;
use log::{debug, info};
use rmcp::transport::TokioChildProcess;
use rmcp::ServiceExt;
use std::collections::HashMap;
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
            workspace_dir,
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

        info!(
            "Spawning MCP server '{}' for session '{}'",
            server_name, self.session_id
        );

        // 6. Spawn process with cross-platform command preparation
        // On Windows, this wraps .cmd/.bat files with cmd.exe
        let (final_command, final_args) =
            crate::mcp::utils::command_helper::prepare_command(command, args);

        debug!("Final spawn command: {} {:?}", final_command, final_args);

        // Determine working directory: use the session workspace directory
        let working_dir = &self.workspace_dir;
        debug!("Spawning MCP server with CWD: {:?}", working_dir);

        let mut cmd = Command::new(&final_command);
        cmd.current_dir(working_dir);
        for arg in &final_args {
            cmd.arg(arg);
        }

        // Apply environment isolation:
        // 1. Clear all inherited environment variables to prevent secret leakage
        cmd.env_clear();

        // 2. Re-apply whitelisted system variables
        for (k, v) in crate::mcp::utils::env::get_isolated_env() {
            cmd.env(k, v);
        }

        // 3. Apply user-defined variables from config (can override system vars)
        for (key, value) in env {
            cmd.env(key, value);
        }

        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let transport = TokioChildProcess::new(cmd)
            .map_err(|e| SessionMCPError::SpawnFailed(format!("{}", e)))?;

        debug!("Created transport for command: {} {:?}", command, args);

        // 7. Initialize rmcp client with timeout
        let timeout = Duration::from_secs(self.config.process_startup_timeout_seconds);
        let client = tokio::time::timeout(timeout, ().serve(transport))
            .await
            .map_err(|_| SessionMCPError::InitTimeout(server_name.to_string()))?
            .map_err(|e| SessionMCPError::InitFailed(format!("{}", e)))?;

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
