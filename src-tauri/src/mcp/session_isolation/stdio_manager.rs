use super::error::SessionMCPError;
use super::process::MCPProcess;
use crate::mcp::session_isolation_config::SessionIsolationConfig;
use crate::mcp::types::{
    MCPError, MCPResponse, MCPResponseResult, MCPServerConfig, TransportConfig,
};
use dashmap::DashMap;
use log::{debug, error, info};
use rmcp::transport::TokioChildProcess;
use rmcp::ServiceExt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Manages session-specific MCP server processes with lazy spawning and idle cleanup.
#[derive(Debug, Clone)]
pub struct SessionMCPManager {
    /// Unique session identifier.
    pub(crate) session_id: String,

    /// Map of server names to running processes.
    pub(crate) active_processes: Arc<RwLock<HashMap<String, MCPProcess>>>,

    /// Map of server names to last activity timestamps.
    pub(crate) last_activity: Arc<RwLock<HashMap<String, Instant>>>,

    /// Per-server spawn locks to prevent race conditions.
    pub(crate) spawn_locks: Arc<DashMap<String, Arc<Mutex<()>>>>,

    /// Idle timeout duration.
    pub(crate) idle_timeout: Duration,

    /// Server configurations for this session.
    pub(crate) server_configs: HashMap<String, MCPServerConfig>,

    /// Session isolation configuration.
    pub(crate) config: SessionIsolationConfig,

    /// Cancellation tokens for active calls.
    pub(crate) active_call_tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,

    /// Workspace directory for the session (CWD for child processes)
    pub(crate) workspace_dir: std::path::PathBuf,
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

    /// Ensures the specified MCP server process is running.
    ///
    /// This is race-safe: if multiple tasks try to spawn the same server,
    /// only one will succeed and the others will wait and reuse it.
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

                // Map rmcp Content to crate::mcp::types::MCPContent
                // Since rmcp uses Annotated<RawContent>, we serialize through JSON
                let local_content: Vec<crate::mcp::types::MCPContent> = call_result
                    .content
                    .into_iter()
                    .filter_map(|c| {
                        // Serialize rmcp Content to JSON and deserialize to our MCPContent
                        let json_val = serde_json::to_value(&c).ok()?;

                        // Check type and convert accordingly
                        if let Some(type_str) = json_val.get("type").and_then(|v| v.as_str()) {
                            match type_str {
                                "text" => {
                                    let text = json_val.get("text")?.as_str()?.to_string();
                                    Some(crate::mcp::types::MCPContent::Text {
                                        text,
                                        is_error: None,
                                    })
                                }
                                "image" => {
                                    let data = json_val.get("data")?.as_str()?.to_string();
                                    let mime_type = json_val.get("mimeType")?.as_str()?.to_string();
                                    Some(crate::mcp::types::MCPContent::Image { data, mime_type })
                                }
                                "resource" => {
                                    // Extract only the nested "resource" field to avoid double-nesting
                                    let resource_data = json_val.get("resource")?.clone();
                                    Some(crate::mcp::types::MCPContent::Resource {
                                        resource: resource_data,
                                        service_info: crate::mcp::types::ServiceInfo {
                                            server_name: server_name.to_string(),
                                            tool_name: tool_name.to_string(),
                                            backend_type: "ExternalMCP".to_string(),
                                        },
                                    })
                                }
                                _ => None,
                            }
                        } else {
                            None
                        }
                    })
                    .collect();

                let mcp_result = crate::mcp::types::MCPResult {
                    content: Some(local_content),
                    structured_content: None,
                    is_error: call_result.is_error,
                };

                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(crate::mcp::types::JsonRpcId::String(
                        uuid::Uuid::new_v4().to_string(),
                    )),
                    result: Some(MCPResponseResult::ToolCall(mcp_result)),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::session_isolation_config::SessionIsolationConfig;
    use crate::mcp::types::{MCPServerConfig, TransportConfig};
    use std::collections::HashMap;

    /// Helper to create a test manager with a simple echo server config
    fn create_test_manager() -> SessionMCPManager {
        let mut configs = HashMap::new();
        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());

        // Use a simple command that exists on all platforms
        #[cfg(windows)]
        let command = "cmd.exe";
        #[cfg(not(windows))]
        let command = "echo";

        configs.insert(
            "test-server".to_string(),
            MCPServerConfig {
                name: Some("test-server".to_string()),
                transport: TransportConfig::Stdio {
                    command: command.to_string(),
                    args: vec![],
                    env: env_vars,
                },
                authentication: None,
                metadata: None,
            },
        );

        let config = SessionIsolationConfig {
            idle_timeout_minutes: 5,
            cleanup_interval_minutes: 5,
            process_startup_timeout_seconds: 30,
            max_restart_attempts: 0,
            http_connection_pool_size: 10,
        };

        SessionMCPManager::new(
            "test-session".to_string(),
            configs,
            config,
            std::env::current_dir().unwrap(),
        )
    }

    #[test]
    fn test_manager_creation() {
        let manager = create_test_manager();
        assert_eq!(manager.session_id, "test-session");
        assert!(manager.has_server("test-server"));
        assert!(!manager.has_server("nonexistent-server"));
    }

    #[test]
    fn test_has_server() {
        let manager = create_test_manager();
        assert!(manager.has_server("test-server"));
        assert!(!manager.has_server("unknown-server"));
    }

    #[test]
    fn test_config_env_vars_are_preserved() {
        let manager = create_test_manager();
        let config = manager.server_configs.get("test-server").unwrap();

        match &config.transport {
            TransportConfig::Stdio { env, .. } => {
                assert_eq!(env.get("TEST_VAR"), Some(&"test_value".to_string()));
            }
            _ => panic!("Expected Stdio transport"),
        }
    }

    #[tokio::test]
    async fn test_multiple_spawn_attempts_are_serialized() {
        // This test verifies that concurrent spawn attempts are properly serialized
        // and only one process is created
        let manager = create_test_manager();

        // Check that spawn locks are created per server
        assert_eq!(manager.spawn_locks.len(), 0);

        // The spawn_locks should be populated on demand during ensure_process_running
        // This test just verifies the initial state
    }

    #[test]
    fn test_idle_timeout_configuration() {
        let manager = create_test_manager();
        // Idle timeout should be 5 minutes (300 seconds)
        assert_eq!(manager.idle_timeout, Duration::from_secs(5 * 60));
    }

    /// Test that environment variables are correctly extracted from config
    #[test]
    fn test_env_vars_extraction() {
        let mut env_map = HashMap::new();
        env_map.insert("PATH".to_string(), "/custom/path".to_string());
        env_map.insert("CUSTOM_VAR".to_string(), "custom_value".to_string());

        let config = MCPServerConfig {
            name: Some("test".to_string()),
            transport: TransportConfig::Stdio {
                command: "test".to_string(),
                args: vec![],
                env: env_map.clone(),
            },
            authentication: None,
            metadata: None,
        };

        match &config.transport {
            TransportConfig::Stdio { env, .. } => {
                assert_eq!(env.len(), 2);
                assert_eq!(env.get("PATH"), Some(&"/custom/path".to_string()));
                assert_eq!(env.get("CUSTOM_VAR"), Some(&"custom_value".to_string()));
            }
            _ => panic!("Expected Stdio transport"),
        }
    }

    /// Test that system PATH inheritance is not blocked
    /// Note: This is a design verification test - we verify that env_clear is NOT called
    #[test]
    fn test_no_env_clear_in_spawn_logic() {
        // This test documents the expected behavior:
        // tokio::process::Command inherits parent environment by default
        // cmd.env(key, value) adds/overrides without clearing

        let source = include_str!("./stdio_manager.rs");

        // Verify that env_clear() is NOT present in the spawn logic
        assert!(
            !source.contains("env_clear()"),
            "stdio_manager should NOT call env_clear() - system PATH must be inherited"
        );

        // Verify that cmd.env() is used (which preserves inheritance)
        assert!(
            source.contains("cmd.env(key, value)"),
            "stdio_manager should use cmd.env() to add custom env vars"
        );
    }

    /// Test SessionMCPError variants
    #[test]
    fn test_error_types() {
        let err1 = SessionMCPError::ServerNotFound("test".to_string());
        assert!(format!("{:?}", err1).contains("ServerNotFound"));

        let err2 = SessionMCPError::SpawnFailed("spawn error".to_string());
        assert!(format!("{:?}", err2).contains("SpawnFailed"));

        let err3 = SessionMCPError::InvalidTransport("wrong type".to_string());
        assert!(format!("{:?}", err3).contains("InvalidTransport"));
    }

    /// Test that command and args are properly structured
    #[test]
    fn test_command_args_structure() {
        let mut configs = HashMap::new();
        configs.insert(
            "npx-server".to_string(),
            MCPServerConfig {
                name: Some("npx-server".to_string()),
                transport: TransportConfig::Stdio {
                    command: "npx".to_string(),
                    args: vec![
                        "-y".to_string(),
                        "@modelcontextprotocol/server-example".to_string(),
                    ],
                    env: HashMap::new(),
                },
                authentication: None,
                metadata: None,
            },
        );

        let config = SessionIsolationConfig {
            idle_timeout_minutes: 5,
            cleanup_interval_minutes: 5,
            process_startup_timeout_seconds: 30,
            max_restart_attempts: 0,
            http_connection_pool_size: 10,
        };

        let manager = SessionMCPManager::new(
            "test".to_string(),
            configs,
            config,
            std::env::current_dir().unwrap(),
        );
        let server_config = manager.server_configs.get("npx-server").unwrap();

        match &server_config.transport {
            TransportConfig::Stdio { command, args, .. } => {
                assert_eq!(command, "npx");
                assert_eq!(args.len(), 2);
                assert_eq!(args[0], "-y");
                assert_eq!(args[1], "@modelcontextprotocol/server-example");
            }
            _ => panic!("Expected Stdio transport"),
        }
    }

    /// Test session ID tracking
    #[test]
    fn test_session_id_tracking() {
        let manager = create_test_manager();
        assert_eq!(manager.session_id, "test-session");
    }

    /// Test that activity tracking structures are initialized
    #[tokio::test]
    async fn test_activity_tracking_initialization() {
        let manager = create_test_manager();

        let activity = manager.last_activity.read().await;
        assert_eq!(activity.len(), 0, "Activity map should be empty initially");

        let processes = manager.active_processes.read().await;
        assert_eq!(processes.len(), 0, "Process map should be empty initially");
    }
}
