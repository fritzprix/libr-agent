use anyhow::Result;
use log::{debug, error, info, warn};
use rmcp::{
    model::CallToolRequestParam,
    transport::{ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess},
    ServiceExt,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::mcp::schema::JSONSchemaType;
use crate::mcp::types::{
    MCPConnection, MCPError, MCPResponse, MCPServerConfig, MCPServerConfigV2, MCPTool,
    SamplingRequest, ServiceContext, ServiceContextOptions, TransportConfig,
};
use crate::session::SessionManager;

/// Manages the lifecycle and communication with both external and built-in MCP servers.
#[derive(Debug)]
pub struct MCPServerManager {
    /// A map of active connections to external MCP servers, keyed by server name.
    connections: Arc<Mutex<HashMap<String, MCPConnection>>>,
    /// A registry for the built-in MCP servers.
    builtin_servers: Arc<Mutex<Option<crate::mcp::builtin::BuiltinServerRegistry>>>,
    /// OAuth manager for handling OAuth 2.1 flows.
    oauth_manager: Arc<crate::mcp::oauth::OAuthManager>,
}

impl MCPServerManager {
    /// Creates a new `MCPServerManager` and initializes the built-in servers
    /// with a reference to the `SessionManager`.
    ///
    /// # Arguments
    /// * `session_manager` - A shared reference to the `SessionManager`.
    pub fn new_with_session_manager(session_manager: Arc<SessionManager>) -> Self {
        let server_manager = Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            builtin_servers: Arc::new(Mutex::new(None)),
            oauth_manager: Arc::new(crate::mcp::oauth::OAuthManager::new()),
        };

        // Initialize builtin servers immediately with SessionManager
        let builtin_registry =
            crate::mcp::builtin::BuiltinServerRegistry::new_with_session_manager(session_manager);
        *server_manager
            .builtin_servers
            .try_lock()
            .expect("Failed to initialize builtin servers") = Some(builtin_registry);
        info!("Initialized MCPServerManager with SessionManager-based builtin servers");

        server_manager
    }

    /// Creates a new `MCPServerManager` with support for both `SessionManager` and SQLite.
    ///
    /// # Arguments
    /// * `session_manager` - A shared reference to the `SessionManager`.
    /// * `sqlite_db_url` - The connection URL for the SQLite database.
    pub async fn new_with_session_manager_and_sqlite(
        session_manager: Arc<SessionManager>,
        sqlite_db_url: String,
    ) -> Self {
        let server_manager = Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            builtin_servers: Arc::new(Mutex::new(None)),
            oauth_manager: Arc::new(crate::mcp::oauth::OAuthManager::new()),
        };

        // Initialize builtin servers with SessionManager and SQLite support
        let builtin_registry =
            crate::mcp::builtin::BuiltinServerRegistry::new_with_session_manager_and_sqlite(
                session_manager,
                sqlite_db_url,
            )
            .await;
        *server_manager
            .builtin_servers
            .try_lock()
            .expect("Failed to initialize builtin servers") = Some(builtin_registry);
        info!("Initialized MCPServerManager with SessionManager and SQLite support");

        server_manager
    }

    /// Starts and connects to an MCP server based on the provided V2 configuration.
    ///
    /// Supports both stdio and HTTP transports with the new MCP 2025-06-18 spec.
    ///
    /// # Arguments
    /// * `config` - The V2 configuration for the server to start.
    ///
    /// # Returns
    /// A `Result` containing a success message, or an error if the transport is unsupported.
    pub async fn start_server_v2(&self, config: MCPServerConfigV2) -> Result<String> {
        match &config.transport {
            TransportConfig::Stdio { command, args, env } => {
                // Convert to legacy format for stdio
                let legacy_config = MCPServerConfig {
                    name: config.name.clone(),
                    command: Some(command.clone()),
                    args: Some(args.clone()),
                    env: Some(env.clone()),
                    transport: "stdio".to_string(),
                    url: None,
                    port: None,
                };
                self.start_stdio_server(legacy_config).await
            }
            TransportConfig::Http {
                url,
                protocol_version,
                session_id,
                headers,
                ..
            } => {
                self.start_http_server(
                    config.name.clone(),
                    url.clone(),
                    protocol_version.clone(),
                    session_id.clone(),
                    headers.clone(),
                )
                .await
            }
        }
    }

    /// Starts and connects to an MCP server based on the provided legacy configuration.
    ///
    /// This is maintained for backward compatibility. New code should use `start_server_v2`.
    ///
    /// # Arguments
    /// * `config` - The legacy configuration for the server to start.
    ///
    /// # Returns
    /// A `Result` containing a success message, or an error if the transport is unsupported.
    pub async fn start_server(&self, config: MCPServerConfig) -> Result<String> {
        match config.transport.as_str() {
            "stdio" => self.start_stdio_server(config).await,
            "http" => {
                // Convert to V2 and use new HTTP handler
                if let Some(url) = config.url {
                    self.start_http_server(config.name, url, "2025-06-18".to_string(), None, None)
                        .await
                } else {
                    Err(anyhow::anyhow!("HTTP transport requires URL"))
                }
            }
            "websocket" => Err(anyhow::anyhow!("WebSocket transport not yet implemented")),
            _ => Err(anyhow::anyhow!(
                "Unsupported transport: {}",
                config.transport
            )),
        }
    }

    /// Starts a new MCP server that communicates over stdio.
    ///
    /// # Arguments
    /// * `config` - The server configuration, must specify a command.
    ///
    /// # Returns
    /// A `Result` containing a success message, or an error on failure.
    async fn start_stdio_server(&self, config: MCPServerConfig) -> Result<String> {
        let command = config
            .command
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Command is required for stdio transport"))?;

        let default_args = vec![];
        let args = config.args.as_ref().unwrap_or(&default_args);

        // Create command with rmcp - configure returns the modified command
        let cmd = Command::new(command).configure(|cmd| {
            for arg in args {
                cmd.arg(arg);
            }

            // Set environment variables if any
            if let Some(env) = &config.env {
                for (key, value) in env {
                    cmd.env(key, value);
                }
            }
        });

        // Create transport and connect using RMCP pattern
        let transport = TokioChildProcess::new(cmd)?;
        debug!("Created transport for command: {command} {args:?}");

        let client = ().serve(transport).await?;
        info!("Successfully connected to MCP server: {}", config.name);

        let connection = MCPConnection { client };

        // Store connection
        {
            let mut connections = self.connections.lock().await;
            connections.insert(config.name.clone(), connection);
            debug!("Stored connection for server: {}", config.name);
        }

        Ok(format!(
            "Started and connected to MCP server: {}",
            config.name
        ))
    }

    /// Starts a new MCP server that communicates over HTTP (Streamable HTTP Transport).
    ///
    /// # Arguments
    /// * `name` - The server name
    /// * `url` - The HTTP endpoint URL
    /// * `protocol_version` - MCP protocol version (default: "2025-06-18")
    /// * `session_id` - Optional existing session ID
    /// * `headers` - Optional custom headers
    ///
    /// # Returns
    /// A `Result` containing a success message, or an error on failure.
    async fn start_http_server(
        &self,
        name: String,
        url: String,
        protocol_version: String,
        _session_id: Option<String>,
        _headers: Option<HashMap<String, String>>,
    ) -> Result<String> {
        info!("Starting HTTP MCP server: {name} at {url}");

        // Create HTTP transport using RMCP's built-in StreamableHttpClientTransport
        let transport = StreamableHttpClientTransport::from_uri(url.clone());

        // Add MCP protocol version header
        // Note: RMCP's HTTP client automatically adds required headers

        debug!("Created HTTP transport for {name} (protocol: {protocol_version})");

        // Connect to the HTTP server
        let client = ().serve(transport).await.map_err(|e| {
            error!("Failed to connect to HTTP MCP server {name}: {e}");
            anyhow::anyhow!("HTTP connection failed: {e}")
        })?;

        info!("Successfully connected to HTTP MCP server: {name}");

        let connection = MCPConnection { client };

        // Store connection
        {
            let mut connections = self.connections.lock().await;
            connections.insert(name.clone(), connection);
            debug!("Stored HTTP connection for server: {name}");
        }

        Ok(format!(
            "Started and connected to HTTP MCP server: {name} at {url}"
        ))
    }

    /// Stops a running MCP server by name.
    ///
    /// # Arguments
    /// * `server_name` - The name of the server to stop.
    pub async fn stop_server(&self, server_name: &str) -> Result<()> {
        let mut connections = self.connections.lock().await;

        if let Some(connection) = connections.remove(server_name) {
            // Cancel the client connection
            let _ = connection.client.cancel().await;
            info!("Stopped MCP server: {server_name}");
        }

        Ok(())
    }

    /// Performs text generation (sampling) on a specified MCP server.
    ///
    /// **Note:** This is currently a placeholder and not fully implemented.
    ///
    /// # Arguments
    /// * `server_name` - The name of the server to use for sampling.
    /// * `request` - The `SamplingRequest` containing the prompt and options.
    ///
    /// # Returns
    /// An `MCPResponse` indicating that the method is not yet implemented.
    pub async fn sample_from_model(
        &self,
        server_name: &str,
        request: SamplingRequest,
    ) -> MCPResponse {
        let connections = self.connections.lock().await;
        let request_id = serde_json::Value::String(Uuid::new_v4().to_string());

        if let Some(_connection) = connections.get(server_name) {
            // This needs to be implemented once RMCP supports sampling.
            // For now, return a temporary error.
            MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32601,
                    message: "Sampling not yet implemented in RMCP".to_string(),
                    data: Some(serde_json::json!({
                        "server_name": server_name,
                        "request": request
                    })),
                }),
            }
        } else {
            MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32002,
                    message: format!("Server '{server_name}' not found"),
                    data: None,
                }),
            }
        }
    }

    /// Calls a tool on a specified MCP server with the given arguments.
    ///
    /// # Arguments
    /// * `server_name` - The name of the server that provides the tool.
    /// * `tool_name` - The name of the tool to call.
    /// * `arguments` - The arguments for the tool, as a `serde_json::Value`.
    ///
    /// # Returns
    /// An `MCPResponse` containing the result or error of the tool call.
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> MCPResponse {
        let connections = self.connections.lock().await;

        // Generate a unique ID for this request
        let request_id = serde_json::Value::String(Uuid::new_v4().to_string());

        if let Some(connection) = connections.get(server_name) {
            // Use the rmcp API - CallToolRequestParam struct
            let args_map = if let serde_json::Value::Object(obj) = arguments {
                obj
            } else {
                serde_json::Map::new()
            };

            let call_param = CallToolRequestParam {
                name: tool_name.to_string().into(),
                arguments: Some(args_map),
            };

            match connection.client.call_tool(call_param).await {
                Ok(result) => {
                    // Log the raw rmcp response first (before serialization)
                    info!("Raw rmcp CallToolResult (before serialization): {result:?}");

                    // Handle the rmcp CallToolResult more carefully
                    let result_value = match serde_json::to_value(&result) {
                        Ok(value) => value,
                        Err(e) => {
                            error!("Failed to serialize tool result: {e}");
                            return MCPResponse {
                                jsonrpc: "2.0".to_string(),
                                id: Some(request_id),
                                result: None,
                                error: Some(MCPError {
                                    code: -32603,
                                    message: format!("Failed to serialize result: {e}"),
                                    data: None,
                                }),
                            };
                        }
                    };

                    // Debug log to check the original structure
                    info!("Original rmcp result: {result:?}");
                    info!("Serialized result: {result_value}");

                    // Detect and add logging for UI resources
                    if let Some(content) = result_value.get("content") {
                        if let Some(content_array) = content.as_array() {
                            for (i, item) in content_array.iter().enumerate() {
                                if item.get("type").and_then(|t| t.as_str()) == Some("resource") {
                                    debug!("Found UI resource at index {i}: {item}");
                                    if let Some(resource) = item.get("resource") {
                                        debug!("Resource details: {resource}");
                                        if resource.get("mimeType").is_none() {
                                            warn!("UI resource missing mimeType: {resource}");
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Check if the result contains an error
                    let contains_error = result_value.to_string().to_lowercase().contains("error");

                    if contains_error
                        && result_value
                            .get("isError")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    {
                        // If isError is true, treat it as an error
                        let error_msg = result_value
                            .get("content")
                            .and_then(|c| c.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|item| item.get("text"))
                            .and_then(|text| text.as_str())
                            .unwrap_or("Tool execution error");

                        MCPResponse::error(request_id, -32000, error_msg)
                    } else {
                        // Normal response - preserve the rmcp structure as much as possible
                        MCPResponse {
                            jsonrpc: "2.0".to_string(),
                            id: Some(request_id),
                            result: Some(result_value),
                            error: None,
                        }
                    }
                }
                Err(e) => {
                    error!("Error calling tool '{tool_name}': {e}");
                    MCPResponse {
                        jsonrpc: "2.0".to_string(),
                        id: Some(request_id),
                        result: None,
                        error: Some(MCPError {
                            code: -32603, // Internal error
                            message: e.to_string(),
                            data: None,
                        }),
                    }
                }
            }
        } else {
            error!("Server '{server_name}' not found");
            MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request_id),
                result: None,
                error: Some(MCPError {
                    code: -32601, // Method not found
                    message: format!("Server '{server_name}' not found"),
                    data: None,
                }),
            }
        }
    }

    /// Lists all tools available on a specific MCP server.
    ///
    /// # Arguments
    /// * `server_name` - The name of the server.
    ///
    /// # Returns
    /// A `Result` containing a vector of `MCPTool` objects, or an error on failure.
    pub async fn list_tools(&self, server_name: &str) -> Result<Vec<MCPTool>> {
        let connections = self.connections.lock().await;

        if let Some(connection) = connections.get(server_name) {
            debug!("Found connection for server: {server_name}");

            match connection.client.list_all_tools().await {
                Ok(tools_response) => {
                    debug!("Raw tools response: {tools_response:?}");
                    let mut tools = Vec::new();

                    for tool in tools_response {
                        debug!("Processing tool: {tool:?}");

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

                        let mcp_tool = MCPTool {
                            name: tool.name.to_string(),
                            title: None,
                            description: tool.description.unwrap_or_default().to_string(),
                            input_schema: structured_schema,
                            output_schema: None,
                            annotations: None,
                        };

                        debug!(
                            "Converted tool: {} with schema type: {:?}",
                            mcp_tool.name, mcp_tool.input_schema.schema_type
                        );
                        tools.push(mcp_tool);
                    }

                    debug!("Successfully converted {} tools", tools.len());
                    Ok(tools)
                }
                Err(e) => {
                    error!("Error listing tools: {e}");
                    Err(anyhow::anyhow!("Failed to list tools: {e}"))
                }
            }
        } else {
            warn!("Server '{server_name}' not found in connections");
            Err(anyhow::anyhow!("Server '{server_name}' not found"))
        }
    }

    /// Lists all tools from all connected MCP servers.
    ///
    /// This method iterates through all active connections, fetches their tools,
    /// and prefixes each tool's name with the server name to avoid conflicts.
    ///
    /// # Returns
    /// A `Result` containing a vector of all `MCPTool` objects from all servers.
    pub async fn list_all_tools(&self) -> Result<Vec<MCPTool>> {
        let mut all_tools = Vec::new();
        let server_names: Vec<String> = {
            let connections = self.connections.lock().await;
            connections.keys().cloned().collect()
        };

        for server_name in server_names {
            match self.list_tools(&server_name).await {
                Ok(mut tools) => {
                    // Prefix tool names with server name to avoid conflicts
                    for tool in &mut tools {
                        tool.name = format!("{}__{}", server_name, tool.name);
                    }
                    all_tools.extend(tools);
                }
                Err(e) => {
                    warn!("Failed to get tools from server {server_name}: {e}");
                    // Continue with other servers instead of failing completely
                }
            }
        }

        Ok(all_tools)
    }

    /// Returns a list of names of all currently connected external MCP servers.
    pub async fn get_connected_servers(&self) -> Vec<String> {
        let connections = self.connections.lock().await;
        connections.keys().cloned().collect()
    }

    /// Checks if a specific external server is currently connected.
    ///
    /// # Arguments
    /// * `server_name` - The name of the server to check.
    ///
    /// # Returns
    /// `true` if the server is connected, `false` otherwise.
    pub async fn is_server_alive(&self, server_name: &str) -> bool {
        let connections = self.connections.lock().await;
        connections.contains_key(server_name)
    }

    /// Checks the status of all connected external servers.
    ///
    /// # Returns
    /// A `HashMap` mapping server names to their connection status (always `true` for connected servers).
    pub async fn check_all_servers(&self) -> HashMap<String, bool> {
        let connections = self.connections.lock().await;
        let mut status_map = HashMap::new();

        for server_name in connections.keys() {
            status_map.insert(server_name.to_string(), true);
        }

        status_map
    }

    /// Validates that a tool's input schema is compatible with AI service expectations.
    ///
    /// This function checks that the schema is of type `object` and that all `required`
    /// fields are defined in the `properties`.
    ///
    /// # Arguments
    /// * `tool` - A reference to the `MCPTool` to validate.
    ///
    /// # Returns
    /// An empty `Result` on success, or an error if validation fails.
    pub fn validate_tool_schema(tool: &MCPTool) -> Result<()> {
        // Ensure the schema type is 'object'
        match &tool.input_schema.schema_type {
            JSONSchemaType::Object {
                properties,
                required,
                ..
            } => {
                // Validate required fields exist in properties
                if let (Some(required_fields), Some(props)) = (required, properties) {
                    for req_field in required_fields {
                        if !props.contains_key(req_field) {
                            return Err(anyhow::anyhow!(
                                "Tool '{}' requires field '{}' but it's not defined in properties",
                                tool.name,
                                req_field
                            ));
                        }
                    }
                } else if required.is_some() && properties.is_none() {
                    return Err(anyhow::anyhow!(
                        "Tool '{}' has required fields but no properties defined",
                        tool.name
                    ));
                }
                Ok(())
            }
            _ => Err(anyhow::anyhow!(
                "Tool '{}' has invalid schema type, expected 'object'",
                tool.name
            )),
        }
    }

    /// Gets a list of tools from a server that pass schema validation.
    ///
    /// # Arguments
    /// * `server_name` - The name of the server to get validated tools from.
    ///
    /// # Returns
    /// A `Result` containing a vector of validated `MCPTool` objects.
    pub async fn get_validated_tools(&self, server_name: &str) -> Result<Vec<MCPTool>> {
        let tools = self.list_tools(server_name).await?;
        let mut validated_tools = Vec::new();

        for tool in tools {
            match Self::validate_tool_schema(&tool) {
                Ok(()) => {
                    debug!("Tool '{}' passed validation", tool.name);
                    validated_tools.push(tool);
                }
                Err(e) => {
                    warn!("Tool '{}' failed validation: {}", tool.name, e);
                    // Optionally, you could try to fix the schema or skip the tool
                }
            }
        }

        Ok(validated_tools)
    }

    /// Lists the names of all available built-in servers.
    pub async fn list_builtin_servers(&self) -> Vec<String> {
        let servers = self.builtin_servers.lock().await;
        match servers.as_ref() {
            Some(registry) => registry.list_servers(),
            None => Vec::new(),
        }
    }

    /// Lists all tools from all available built-in servers.
    pub async fn list_builtin_tools(&self) -> Vec<MCPTool> {
        let servers = self.builtin_servers.lock().await;
        match servers.as_ref() {
            Some(registry) => registry.list_all_tools(),
            None => Vec::new(),
        }
    }

    /// Lists the tools for a specific built-in server.
    ///
    /// # Arguments
    /// * `server_name` - The name of the built-in server.
    pub async fn list_builtin_tools_for(&self, server_name: &str) -> Vec<MCPTool> {
        let servers = self.builtin_servers.lock().await;
        match servers.as_ref() {
            Some(registry) => registry.list_tools_for_server(server_name),
            None => Vec::new(),
        }
    }

    /// Calls a tool on a built-in server.
    ///
    /// # Arguments
    /// * `server_name` - The name of the built-in server.
    /// * `tool_name` - The name of the tool to call.
    /// * `args` - The arguments for the tool, as a `serde_json::Value`.
    ///
    /// # Returns
    /// An `MCPResponse` containing the result of the tool call.
    pub async fn call_builtin_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> MCPResponse {
        debug!(
            "call_builtin_tool: server_name='{server_name}', tool_name='{tool_name}', args={args}"
        );

        let servers = self.builtin_servers.lock().await;
        let result = match servers.as_ref() {
            Some(registry) => registry.call_tool(server_name, tool_name, args).await,
            None => {
                let request_id = serde_json::Value::String(Uuid::new_v4().to_string());
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32001,
                        message: "Builtin servers not initialized".to_string(),
                        data: None,
                    }),
                }
            }
        };

        debug!(
            "Builtin tool call result: success={}",
            result.error.is_none()
        );

        result
    }

    /// Gets a unified list of all tools from both external and built-in servers.
    pub async fn list_all_tools_unified(&self) -> Result<Vec<MCPTool>> {
        let mut all_tools = Vec::new();

        // Get external server tools
        match self.list_all_tools().await {
            Ok(external_tools) => all_tools.extend(external_tools),
            Err(e) => warn!("Failed to get external server tools: {e}"),
        }

        // Get builtin server tools
        let builtin_tools = self.list_builtin_tools().await;
        all_tools.extend(builtin_tools);

        Ok(all_tools)
    }

    /// Calls a tool, automatically routing the request to either a built-in or an
    /// external server based on the server name prefix.
    ///
    /// # Arguments
    /// * `server_name` - The name of the server. If it starts with "builtin.", it's
    ///   routed to the built-in server registry.
    /// * `tool_name` - The name of the tool to call.
    /// * `args` - The arguments for the tool.
    ///
    /// # Returns
    /// An `MCPResponse` from the appropriate server.
    pub async fn call_tool_unified(
        &self,
        server_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> MCPResponse {
        // Check if it's a builtin server (starts with "builtin.")
        if server_name.starts_with("builtin.") {
            let normalized_server_name =
                server_name.strip_prefix("builtin.").unwrap_or(server_name);
            self.call_builtin_tool(normalized_server_name, tool_name, args)
                .await
        } else {
            self.call_tool(server_name, tool_name, args).await
        }
    }

    /// Gets the service context for a given server, checking built-in servers first.
    ///
    /// # Arguments
    /// * `server_name` - The name of the server.
    /// * `options` - Optional context options for the service.
    ///
    /// # Returns
    /// A `Result` containing the service context, or an error.
    pub async fn get_service_context(
        &self,
        server_name: &str,
        options: Option<ServiceContextOptions>,
    ) -> Result<ServiceContext, String> {
        // Check built-in servers first
        let servers = self.builtin_servers.lock().await;
        if let Some(registry) = servers.as_ref() {
            if let Ok(context) = registry.get_server_context(
                server_name,
                options.map(|o| serde_json::to_value(o).unwrap_or(Value::Null)),
            ) {
                return Ok(context);
            }
        }

        // Fallback for external MCP servers (future implementation)
        Ok(ServiceContext {
            context_prompt: format!(
                "# MCP Server Context\nServer ID: {server_name}\nStatus: Active"
            ),
            structured_state: None,
        })
    }

    /// Switches the context for a given server.
    ///
    /// # Arguments
    /// * `server_name` - The name of the server.
    /// * `options` - The context options to switch to.
    ///
    /// # Returns
    /// A `Result` indicating success or an error.
    #[allow(dead_code)]
    pub async fn switch_context(
        &self,
        server_name: &str,
        options: ServiceContextOptions,
    ) -> Result<(), String> {
        // Check built-in servers first
        let servers = self.builtin_servers.lock().await;
        if let Some(registry) = servers.as_ref() {
            return registry.switch_server_context(server_name, options).await;
        }

        // Fallback for external MCP servers (future implementation)
        info!("Switching context for external server: {server_name} (not implemented)");
        Ok(())
    }

    /// Returns a reference to the OAuth manager for handling OAuth 2.1 flows.
    ///
    /// # Returns
    /// An `Arc` reference to the `OAuthManager`.
    pub async fn get_oauth_manager(&self) -> Arc<crate::mcp::oauth::OAuthManager> {
        Arc::clone(&self.oauth_manager)
    }
}
