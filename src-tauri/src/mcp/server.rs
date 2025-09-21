use anyhow::Result;
use log::{debug, error, info, warn};
use rmcp::{
    model::CallToolRequestParam,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::mcp::schema::JSONSchemaType;
use crate::mcp::types::{
    MCPConnection, MCPError, MCPResponse, MCPServerConfig, MCPTool, SamplingRequest,
};
use crate::session::SessionManager;

#[derive(Debug)]
pub struct MCPServerManager {
    connections: Arc<Mutex<HashMap<String, MCPConnection>>>,
    builtin_servers: Arc<Mutex<Option<crate::mcp::builtin::BuiltinServerRegistry>>>,
}

impl MCPServerManager {
    pub fn new_with_session_manager(session_manager: Arc<SessionManager>) -> Self {
        let server_manager = Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            builtin_servers: Arc::new(Mutex::new(None)),
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

    /// SQLite 지원을 위한 MCP 서버 매니저 생성
    pub async fn new_with_session_manager_and_sqlite(
        session_manager: Arc<SessionManager>,
        sqlite_db_url: String,
    ) -> Self {
        let server_manager = Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            builtin_servers: Arc::new(Mutex::new(None)),
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

    /// MCP 서버를 시작하고 연결합니다
    pub async fn start_server(&self, config: MCPServerConfig) -> Result<String> {
        match config.transport.as_str() {
            "stdio" => self.start_stdio_server(config).await,
            "http" => {
                // HTTP 서버는 외부에서 이미 실행 중이라고 가정
                Ok(format!("HTTP server configured: {}", config.name))
            }
            "websocket" => {
                // WebSocket 서버는 외부에서 이미 실행 중이라고 가정
                Ok(format!("WebSocket server configured: {}", config.name))
            }
            _ => Err(anyhow::anyhow!(
                "Unsupported transport: {}",
                config.transport
            )),
        }
    }

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

    /// MCP 서버를 중지합니다
    pub async fn stop_server(&self, server_name: &str) -> Result<()> {
        let mut connections = self.connections.lock().await;

        if let Some(connection) = connections.remove(server_name) {
            // Cancel the client connection
            let _ = connection.client.cancel().await;
            info!("Stopped MCP server: {server_name}");
        }

        Ok(())
    }

    /// MCP 서버에서 sampling을 수행합니다
    pub async fn sample_from_model(
        &self,
        server_name: &str,
        request: SamplingRequest,
    ) -> MCPResponse {
        let connections = self.connections.lock().await;
        let request_id = serde_json::Value::String(Uuid::new_v4().to_string());

        if let Some(_connection) = connections.get(server_name) {
            // RMCP에서 sampling 지원 여부 확인 후 구현
            // 현재는 임시로 에러 반환
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

    /// 도구를 호출합니다
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
            // RMCP API 사용 - CallToolRequestParam 구조체 사용
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
                    // RMCP 원본 응답을 먼저 로깅 (직렬화 전)
                    info!("Raw rmcp CallToolResult (before serialization): {result:?}");

                    // rmcp CallToolResult를 보다 신중하게 처리
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

                    // 디버그 로그로 원본 구조 확인
                    info!("Original rmcp result: {result:?}");
                    info!("Serialized result: {result_value}");

                    // UI 리소스 감지 및 추가 로깅
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

                    // 결과에 에러가 포함되어 있는지 확인
                    let contains_error = result_value.to_string().to_lowercase().contains("error");

                    if contains_error
                        && result_value
                            .get("isError")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    {
                        // isError가 true인 경우 에러로 처리
                        let error_msg = result_value
                            .get("content")
                            .and_then(|c| c.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|item| item.get("text"))
                            .and_then(|text| text.as_str())
                            .unwrap_or("Tool execution error");

                        MCPResponse::error(request_id, -32000, error_msg)
                    } else {
                        // 정상 응답 - rmcp 구조를 최대한 보존
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

    /// 사용 가능한 도구 목록을 가져옵니다
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

    /// Get tools from all connected servers
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

    /// 연결된 서버 목록을 반환합니다
    pub async fn get_connected_servers(&self) -> Vec<String> {
        let connections = self.connections.lock().await;
        connections.keys().cloned().collect()
    }

    /// 특정 서버가 연결되어 있는지 확인합니다
    pub async fn is_server_alive(&self, server_name: &str) -> bool {
        let connections = self.connections.lock().await;
        connections.contains_key(server_name)
    }

    /// 모든 서버의 상태를 확인합니다
    pub async fn check_all_servers(&self) -> HashMap<String, bool> {
        let connections = self.connections.lock().await;
        let mut status_map = HashMap::new();

        for server_name in connections.keys() {
            status_map.insert(server_name.to_string(), true);
        }

        status_map
    }

    /// Validate if a tool schema is compatible with AI service expectations
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

    /// Get validated tools that are compatible with the AI service
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

    /// Built-in server methods
    /// List all available builtin servers
    pub async fn list_builtin_servers(&self) -> Vec<String> {
        let servers = self.builtin_servers.lock().await;
        match servers.as_ref() {
            Some(registry) => registry.list_servers(),
            None => Vec::new(),
        }
    }

    /// List all tools from builtin servers
    pub async fn list_builtin_tools(&self) -> Vec<MCPTool> {
        let servers = self.builtin_servers.lock().await;
        match servers.as_ref() {
            Some(registry) => registry.list_all_tools(),
            None => Vec::new(),
        }
    }

    /// List tools from a specific builtin server
    pub async fn list_builtin_tools_for(&self, server_name: &str) -> Vec<MCPTool> {
        let servers = self.builtin_servers.lock().await;
        match servers.as_ref() {
            Some(registry) => registry.list_tools_for_server(server_name),
            None => Vec::new(),
        }
    }

    /// Call a tool on a builtin server
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

    /// Get unified list of tools from both external and builtin servers
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

    /// Call a tool on either external or builtin server based on server name
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

    pub async fn get_service_context(&self, server_name: &str) -> Result<String, String> {
        // Check built-in servers first
        let servers = self.builtin_servers.lock().await;
        if let Some(registry) = servers.as_ref() {
            if let Ok(context) = registry.get_server_context(server_name, None) {
                return Ok(context);
            }
        }

        // Fallback for external MCP servers (future implementation)
        Ok(format!(
            "# MCP Server Context\nServer ID: {server_name}\nStatus: Active"
        ))
    }
}
