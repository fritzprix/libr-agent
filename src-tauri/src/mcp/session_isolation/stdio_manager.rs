use super::error::SessionMCPError;
use super::process::MCPProcess;
use crate::mcp::session_isolation_config::SessionIsolationConfig;
use crate::mcp::types::{
    MCPError, MCPResponse, MCPResponseResult, MCPServerConfig, TransportConfig,
};
use dashmap::DashMap;
use log::{debug, error, info};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::ServiceExt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

/// Manages session-specific MCP server processes with lazy spawning and idle cleanup.
#[derive(Debug, Clone)]
pub struct SessionMCPManager {
    /// Unique session identifier.
    session_id: String,

    /// Map of server names to running processes.
    active_processes: Arc<RwLock<HashMap<String, MCPProcess>>>,

    /// Map of server names to last activity timestamps.
    last_activity: Arc<RwLock<HashMap<String, Instant>>>,

    /// Per-server spawn locks to prevent race conditions.
    spawn_locks: Arc<DashMap<String, Arc<Mutex<()>>>>,

    /// Idle timeout duration.
    idle_timeout: Duration,

    /// Server configurations for this session.
    server_configs: HashMap<String, MCPServerConfig>,

    /// Session isolation configuration.
    config: SessionIsolationConfig,

    /// Cancellation tokens for active calls.
    active_call_tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,
}

impl SessionMCPManager {
    /// Checks if a server is managed by this session manager
    pub fn has_server(&self, server_name: &str) -> bool {
        self.server_configs.contains_key(server_name)
    }

    /// Creates a new SessionMCPManager for the given session.
    pub fn new(
        session_id: String,
        server_configs: HashMap<String, MCPServerConfig>,
        config: SessionIsolationConfig,
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
        }
    }

    /// Ensures the specified MCP server process is running.
    ///
    /// This is race-safe: if multiple tasks try to spawn the same server,
    /// only one will succeed and the others will wait and reuse it.
    async fn ensure_process_running(&self, server_name: &str) -> Result<(), SessionMCPError> {
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

        // 6. Spawn process
        let cmd = Command::new(command).configure(|cmd| {
            for arg in args {
                cmd.arg(arg);
            }
            for (key, value) in env {
                cmd.env(key, value);
            }
        });

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

        // 8. Store process
        let process = MCPProcess::new(client);

        let mut processes = self.active_processes.write().await;
        processes.insert(server_name.to_string(), process);

        let mut activity = self.last_activity.write().await;
        activity.insert(server_name.to_string(), Instant::now());

        Ok(())
    }

    /// Calls a tool on the specified MCP server.
    ///
    /// This will spawn the process if it's not running, execute the tool call,
    /// and detect crashes.
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: Value,
    ) -> Result<MCPResponse, SessionMCPError> {
        // 1. Ensure process is running
        self.ensure_process_running(server_name).await?;

        // 2. Increment active call counter
        let active_calls_guard = {
            let processes = self.active_processes.read().await;
            let process = processes
                .get(server_name)
                .ok_or_else(|| SessionMCPError::ServerNotFound(server_name.to_string()))?;

            let guard = process.active_calls.clone();
            guard.fetch_add(1, Ordering::Relaxed);
            guard
        };

        // 3. Create cancellation token for this call
        let cancel_token = CancellationToken::new();
        self.active_call_tokens
            .write()
            .await
            .insert(server_name.to_string(), cancel_token.clone());

        // 4. Call tool with cancellation support
        let call_param = rmcp::model::CallToolRequestParam {
            name: tool_name.to_string().into(),
            arguments: args.as_object().cloned(),
        };

        let result = {
            let processes = self.active_processes.read().await;
            let process = processes
                .get(server_name)
                .ok_or_else(|| SessionMCPError::ServerNotFound(server_name.to_string()))?;

            tokio::select! {
                result = process.client.call_tool(call_param) => result,
                _ = cancel_token.cancelled() => {
                    return Err(SessionMCPError::CallCancelled);
                }
            }
        };

        // 5. Handle result and check for crashes
        let mcp_response = match result {
            Ok(call_result) => {
                // Success - update activity timestamp
                self.last_activity
                    .write()
                    .await
                    .insert(server_name.to_string(), Instant::now());

                // Convert to MCPResponse
                let result_value = serde_json::to_value(&call_result)
                    .map_err(|e| SessionMCPError::SerializationError(format!("{}", e)))?;

                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(crate::mcp::types::JsonRpcId::String(
                        uuid::Uuid::new_v4().to_string(),
                    )),
                    result: Some(MCPResponseResult::Generic(result_value)),
                    error: None,
                }
            }
            Err(e) => {
                // Tool call failed - log error and return error response
                error!("MCP server '{}' tool call failed: {}", server_name, e);

                // If the error indicates a connection/communication failure,
                // remove the process from the map so it will be respawned on next call
                let error_msg = format!("{}", e);
                if error_msg.contains("connection")
                    || error_msg.contains("closed")
                    || error_msg.contains("broken pipe")
                {
                    let mut processes = self.active_processes.write().await;
                    processes.remove(server_name);
                    info!(
                        "Removed failed MCP server '{}' - will respawn on next call",
                        server_name
                    );
                }

                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(crate::mcp::types::JsonRpcId::String(
                        uuid::Uuid::new_v4().to_string(),
                    )),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Tool call failed: {}", e),
                        data: None,
                    }),
                }
            }
        };

        // 6. Cleanup
        active_calls_guard.fetch_sub(1, Ordering::Relaxed);
        self.active_call_tokens.write().await.remove(server_name);

        Ok(mcp_response)
    }

    /// List all available tools from a specific MCP server.
    ///
    /// This will spawn the process if it's not running, fetch the tools,
    /// and keep the process alive for subsequent tool calls.
    pub async fn list_tools(
        &self,
        server_name: &str,
    ) -> Result<Vec<crate::mcp::types::MCPTool>, SessionMCPError> {
        use log::warn;

        // Ensure process is running
        self.ensure_process_running(server_name).await?;

        // Fetch tools from the running process
        let processes = self.active_processes.read().await;
        let process = processes
            .get(server_name)
            .ok_or_else(|| SessionMCPError::ServerNotFound(server_name.to_string()))?;

        match process.client.list_all_tools().await {
            Ok(tools_response) => {
                debug!(
                    "Fetched {} tools from server '{}' for session '{}'",
                    tools_response.len(),
                    server_name,
                    self.session_id
                );

                let mut tools = Vec::new();

                for tool in tools_response {
                    // Convert the input schema to our structured format
                    let input_schema_value = serde_json::to_value(tool.input_schema)
                        .unwrap_or_else(|e| {
                            warn!(
                                "Failed to serialize input_schema for tool {}: {}",
                                tool.name, e
                            );
                            serde_json::Value::Object(serde_json::Map::new())
                        });

                    let structured_schema =
                        crate::mcp::server_utils::convert_input_schema(input_schema_value);

                    let mcp_tool = crate::mcp::types::MCPTool {
                        name: tool.name.to_string(),
                        title: None,
                        description: tool.description.unwrap_or_default().to_string(),
                        input_schema: structured_schema,
                        output_schema: None,
                        annotations: None,
                    };

                    tools.push(mcp_tool);
                }

                Ok(tools)
            }
            Err(e) => {
                error!(
                    "Failed to list tools from server '{}' for session '{}': {}",
                    server_name, self.session_id, e
                );
                Err(SessionMCPError::ToolCallFailed(format!(
                    "Failed to list tools: {}",
                    e
                )))
            }
        }
    }

    /// Remove idle processes (called by background task).
    pub async fn cleanup_idle_processes(&self) {
        let now = Instant::now();
        let mut processes = self.active_processes.write().await;
        let activity = self.last_activity.read().await;

        let idle_servers: Vec<String> = activity
            .iter()
            .filter_map(|(name, &last_activity)| {
                // Check idle timeout
                if now.duration_since(last_activity) <= self.idle_timeout {
                    return None;
                }

                // Check if process has active calls
                if let Some(process) = processes.get(name) {
                    if process.active_calls.load(Ordering::Relaxed) > 0 {
                        debug!("Skipping cleanup of '{}' - has active calls", name);
                        return None;
                    }
                }

                Some(name.clone())
            })
            .collect();

        for server_name in idle_servers {
            info!(
                "Terminating idle MCP server '{}' for session '{}'",
                server_name, self.session_id
            );

            if let Some(process) = processes.remove(&server_name) {
                // Spawn cleanup task (don't block)
                tokio::spawn(async move {
                    process.shutdown().await;
                });
            }
        }
    }

    /// Shutdown all processes (called on session destroy).
    pub async fn shutdown_all(&self) {
        info!(
            "Shutting down all MCP processes for session '{}'",
            self.session_id
        );

        // Cancel all active calls
        let tokens = self.active_call_tokens.read().await;
        for token in tokens.values() {
            token.cancel();
        }
        drop(tokens);

        // Wait briefly for calls to abort
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Remove all processes
        let mut processes = self.active_processes.write().await;
        let process_list: Vec<_> = processes.drain().collect();

        // Shutdown in parallel
        let shutdown_tasks: Vec<_> = process_list
            .into_iter()
            .map(|(name, process)| {
                tokio::spawn(async move {
                    debug!("Killing MCP server '{}'", name);
                    process.shutdown().await;
                })
            })
            .collect();

        // Wait for all shutdowns with timeout
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            futures::future::join_all(shutdown_tasks),
        )
        .await;
    }
}
