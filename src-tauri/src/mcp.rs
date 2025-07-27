use anyhow::Result;
use log::{debug, error, info, warn};
use rmcp::{
    model::CallToolRequestParam,
    service::{RoleClient, RunningService},
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerConfig {
    pub name: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    #[serde(default = "default_transport")]
    pub transport: String, // "stdio" | "http" | "websocket"
    pub url: Option<String>,
    pub port: Option<u16>,
}

fn default_transport() -> String {
    "stdio".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolInputSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub properties: serde_json::Map<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    // Allow additional fields for flexibility
    #[serde(flatten)]
    pub additional_properties: serde_json::Map<String, serde_json::Value>,
}

impl Default for MCPToolInputSchema {
    fn default() -> Self {
        Self {
            schema_type: "object".to_string(),
            properties: serde_json::Map::new(),
            required: None,
            description: None,
            title: None,
            additional_properties: serde_json::Map::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPTool {
    pub name: String,
    pub description: String,
    pub input_schema: MCPToolInputSchema,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

pub struct MCPConnection {
    pub client: RunningService<RoleClient, ()>,
}

pub struct MCPServerManager {
    connections: Arc<Mutex<HashMap<String, MCPConnection>>>,
}

impl MCPServerManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
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
        debug!("Created transport for command: {} {:?}", command, args);

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
            info!("Stopped MCP server: {}", server_name);
        }

        Ok(())
    }

    /// 도구를 호출합니다
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> ToolCallResult {
        let connections = self.connections.lock().await;

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
                Ok(result) => ToolCallResult {
                    success: true,
                    result: Some(serde_json::to_value(result).unwrap_or(serde_json::Value::Null)),
                    error: None,
                },
                Err(e) => {
                    error!("Error calling tool '{}': {}", tool_name, e);
                    ToolCallResult {
                        success: false,
                        result: None,
                        error: Some(e.to_string()),
                    }
                },
            }
        } else {
            error!("Server '{}' not found", server_name);
            ToolCallResult {
                success: false,
                result: None,
                error: Some(format!("Server '{}' not found", server_name)),
            }
        }
    }

    /// Convert JSON schema to structured MCPToolInputSchema
    fn convert_input_schema(schema: serde_json::Value) -> MCPToolInputSchema {
        match schema {
            serde_json::Value::Object(obj) => {
                let mut input_schema = MCPToolInputSchema::default();

                // Extract known fields
                if let Some(schema_type) = obj.get("type") {
                    if let Some(type_str) = schema_type.as_str() {
                        input_schema.schema_type = type_str.to_string();
                    }
                }

                if let Some(properties) = obj.get("properties") {
                    if let Some(props_obj) = properties.as_object() {
                        input_schema.properties = props_obj.clone();
                    }
                }

                if let Some(required) = obj.get("required") {
                    if let Some(req_array) = required.as_array() {
                        let required_strings: Vec<String> = req_array
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        if !required_strings.is_empty() {
                            input_schema.required = Some(required_strings);
                        }
                    }
                }

                if let Some(description) = obj.get("description") {
                    if let Some(desc_str) = description.as_str() {
                        input_schema.description = Some(desc_str.to_string());
                    }
                }

                if let Some(title) = obj.get("title") {
                    if let Some(title_str) = title.as_str() {
                        input_schema.title = Some(title_str.to_string());
                    }
                }

                // Store any additional properties
                for (key, value) in obj {
                    if !matches!(
                        key.as_str(),
                        "type" | "properties" | "required" | "description" | "title"
                    ) {
                        input_schema.additional_properties.insert(key, value);
                    }
                }

                input_schema
            }
            _ => {
                // If it's not an object, create a default schema
                warn!("Received non-object schema, using default");
                MCPToolInputSchema::default()
            }
        }
    }

    /// 사용 가능한 도구 목록을 가져옵니다
    pub async fn list_tools(&self, server_name: &str) -> Result<Vec<MCPTool>> {
        let connections = self.connections.lock().await;

        if let Some(connection) = connections.get(server_name) {
            debug!("Found connection for server: {}", server_name);

            match connection.client.list_all_tools().await {
                Ok(tools_response) => {
                    debug!("Raw tools response: {:?}", tools_response);
                    let mut tools = Vec::new();

                    for tool in tools_response {
                        debug!("Processing tool: {:?}", tool);

                        // Convert the input schema to our structured format
                        let input_schema_value = serde_json::to_value(tool.input_schema)
                            .unwrap_or_else(|e| {
                                warn!(
                                    "Failed to serialize input_schema for tool {}: {}",
                                    tool.name, e
                                );
                                serde_json::Value::Object(serde_json::Map::new())
                            });

                        let structured_schema = Self::convert_input_schema(input_schema_value);

                        let mcp_tool = MCPTool {
                            name: tool.name.to_string(),
                            description: tool.description.unwrap_or_default().to_string(),
                            input_schema: structured_schema,
                        };

                        debug!(
                            "Converted tool: {} with schema type: {}",
                            mcp_tool.name, mcp_tool.input_schema.schema_type
                        );
                        tools.push(mcp_tool);
                    }

                    debug!("Successfully converted {} tools", tools.len());
                    Ok(tools)
                }
                Err(e) => {
                    error!("Error listing tools: {}", e);
                    Err(anyhow::anyhow!("Failed to list tools: {}", e))
                }
            }
        } else {
            warn!("Server '{}' not found in connections", server_name);
            Err(anyhow::anyhow!("Server '{}' not found", server_name))
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
                    warn!(
                        "Failed to get tools from server {}: {}",
                        server_name, e
                    );
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
        if tool.input_schema.schema_type != "object" {
            return Err(anyhow::anyhow!(
                "Tool '{}' has invalid schema type '{}', expected 'object' (input_schema: {:?})",
                tool.name,
                tool.input_schema.schema_type,
                tool.input_schema
            ));
        }

        // Validate required fields exist in properties
        if let Some(required) = &tool.input_schema.required {
            if tool.input_schema.properties.is_empty() {
                return Err(anyhow::anyhow!(
                    "Tool '{}' has required fields but no properties defined (input_schema: {:?})",
                    tool.name,
                    tool.input_schema
                ));
            }
            for req_field in required {
                if !tool.input_schema.properties.contains_key(req_field) {
                    return Err(anyhow::anyhow!(
                        "Tool '{}' requires field '{}' but it's not defined in properties (input_schema: {:?})",
                        tool.name,
                        req_field,
                        tool.input_schema
                    ));
                }
            }
        }

        Ok(())
    }

    /// Get validated tools that are compatible with the AI service
    pub async fn get_validated_tools(&self, server_name: &str) -> Result<Vec<MCPTool>> {
        let tools = self.list_tools(server_name).await?;
        let mut validated_tools = Vec::new();

        for tool in tools {
            match Self::validate_tool_schema(&tool) {
                Ok(()) => {
                    println!("Tool '{}' passed validation", tool.name);
                    validated_tools.push(tool);
                }
                Err(e) => {
                    println!("Tool '{}' failed validation: {}", tool.name, e);
                    // Optionally, you could try to fix the schema or skip the tool
                }
            }
        }

        Ok(validated_tools)
    }
}


