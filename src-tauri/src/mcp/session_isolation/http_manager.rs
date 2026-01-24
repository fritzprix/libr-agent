use anyhow::{anyhow, Context};
use log::{debug, error, info, warn};
use rmcp::{
    model::CallToolRequestParam,
    service::{RoleClient, RunningService},
    transport::streamable_http_client::{
        StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
    },
    ServiceExt,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::mcp::types::{MCPResponse, MCPServerConfig, MCPTool, TransportConfig};

use super::error::SessionMCPError;

/// Manages HTTP/SSE MCP server connections with session context injection.
///
/// Unlike stdio servers which need isolated processes, HTTP servers are shared
/// across sessions but inject the session ID via the Mcp-Session-Id header.
#[derive(Debug, Clone)]
pub struct HttpSessionManager {
    /// Unique session identifier
    session_id: String,

    /// Map of server names to their configurations (HTTP only)
    http_configs: Arc<RwLock<HashMap<String, MCPServerConfig>>>,

    /// Active session-specific connections
    connections: Arc<Mutex<HashMap<String, RunningService<RoleClient, ()>>>>,
}

impl HttpSessionManager {
    /// Creates a new HTTP session manager for the given session.
    pub fn new(session_id: String, http_configs: HashMap<String, MCPServerConfig>) -> Self {
        info!(
            "Created HTTP session manager for session '{}' with {} servers",
            session_id,
            http_configs.len()
        );

        Self {
            session_id,
            http_configs: Arc::new(RwLock::new(http_configs)),
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Starts a session-isolated HTTP connection for the given server
    pub async fn start_server(
        &self,
        server_name: &str,
        config: MCPServerConfig,
    ) -> anyhow::Result<()> {
        let (url, headers, enable_sse) = match &config.transport {
            TransportConfig::Http {
                url,
                headers,
                enable_sse,
                ..
            } => (url.clone(), headers.clone(), *enable_sse),
            _ => return Err(anyhow!("Not an HTTP server config")),
        };

        info!(
            "Starting session-isolated HTTP MCP server: {} for session {}",
            server_name, self.session_id
        );

        let mut header_map = reqwest::header::HeaderMap::new();

        // Add custom headers
        if let Some(headers) = headers {
            for (k, v) in headers {
                if let (Ok(k), Ok(v)) = (
                    reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                    reqwest::header::HeaderValue::from_str(&v),
                ) {
                    header_map.insert(k, v);
                } else {
                    error!("Invalid header ignored: {}: {}", k, v);
                }
            }
        }

        // Inject session ID
        if let Ok(v) = reqwest::header::HeaderValue::from_str(&self.session_id) {
            // Mcp-Session-Id header
            if let Ok(k) = reqwest::header::HeaderName::from_bytes(b"Mcp-Session-Id") {
                header_map.insert(k, v);
            }
        }

        // Build reqwest client with fixed headers
        let client = reqwest::Client::builder()
            .default_headers(header_map)
            .build()
            .context("Failed to build HTTP client")?;

        let mut transport_config = StreamableHttpClientTransportConfig::with_uri(url);
        if let Some(sse) = enable_sse {
            transport_config.allow_stateless = !sse;
        }

        let transport = StreamableHttpClientTransport::with_client(client, transport_config);

        let mcp_client = ().serve(transport).await.map_err(|e| {
            error!("Failed to connect to HTTP MCP server {server_name}: {e}");
            anyhow!("HTTP connection failed: {e}")
        })?;

        // Store connection
        {
            let mut connections = self.connections.lock().await;
            connections.insert(server_name.to_string(), mcp_client);
        }

        debug!(
            "Established HTTP connection for server: {} (Session: {})",
            server_name, self.session_id
        );
        Ok(())
    }

    /// Calls a tool on an HTTP MCP server with session context.
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<MCPResponse, SessionMCPError> {
        let mut client_exists = false;
        {
            let connections = self.connections.lock().await;
            if connections.contains_key(server_name) {
                client_exists = true;
            }
        }

        if !client_exists {
            warn!(
                "HTTP connection for {} not found in session {}, attempting to start...",
                server_name, self.session_id
            );
            let config_opt = { self.http_configs.read().await.get(server_name).cloned() };

            if let Some(config) = config_opt {
                if let Err(e) = self.start_server(server_name, config).await {
                    return Err(SessionMCPError::ExecutionError(format!(
                        "Failed to start HTTP server on demand: {e}"
                    )));
                }
            } else {
                return Err(SessionMCPError::ServerNotFound(server_name.to_string()));
            }
        }

        let connections = self.connections.lock().await;
        if let Some(client) = connections.get(server_name) {
            debug!(
                "Calling HTTP tool '{}::{}' using session-isolated connection for '{}'",
                server_name, tool_name, self.session_id
            );

            let args_map = if let serde_json::Value::Object(obj) = args {
                obj
            } else {
                serde_json::Map::new()
            };

            let call_param = CallToolRequestParam {
                name: tool_name.to_string().into(),
                arguments: Some(args_map),
            };

            let result = client
                .call_tool(call_param)
                .await
                .map_err(|e| SessionMCPError::ExecutionError(format!("Tool call failed: {e}")))?;

            // Map rmcp Content to crate::mcp::types::MCPContent
            // Since rmcp uses Annotated<RawContent>, we serialize through JSON
            let local_content: Vec<crate::mcp::types::MCPContent> = result
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
                                Some(crate::mcp::types::MCPContent::Text { text })
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
                is_error: result.is_error,
            };

            let response = MCPResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(crate::mcp::types::MCPResponseResult::ToolCall(mcp_result)),
                id: None,
                error: None,
            };
            return Ok(response);
        }

        Err(SessionMCPError::ConnectionError(format!(
            "No connection for HTTP server: {}",
            server_name
        )))
    }

    /// Fetches tools from a session-isolated HTTP connection
    pub async fn list_tools(&self, server_name: &str) -> anyhow::Result<Vec<MCPTool>> {
        let mut client_exists = false;
        {
            let connections = self.connections.lock().await;
            if connections.contains_key(server_name) {
                client_exists = true;
            }
        }

        if !client_exists {
            warn!(
                "HTTP connection for {} not found in session {} (list_tools), attempting to start...",
                server_name, self.session_id
            );
            let config_opt = { self.http_configs.read().await.get(server_name).cloned() };
            if let Some(config) = config_opt {
                self.start_server(server_name, config).await?;
            } else {
                return Err(anyhow!("Server not configured"));
            }
        }

        let connections = self.connections.lock().await;
        if let Some(client) = connections.get(server_name) {
            let tools_response = client.list_all_tools().await?;
            let mut tools = Vec::new();

            for tool in tools_response {
                let input_schema_value =
                    serde_json::to_value(tool.input_schema).unwrap_or_else(|e| {
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
                tools.push(mcp_tool);
            }
            return Ok(tools);
        }
        Err(anyhow!("Server not connected"))
    }

    /// Get the list of HTTP server names managed by this session.
    pub async fn list_servers(&self) -> Vec<String> {
        self.http_configs.read().await.keys().cloned().collect()
    }

    /// Check if a specific server is managed by this session.
    pub async fn has_server(&self, server_name: &str) -> bool {
        self.http_configs.read().await.contains_key(server_name)
    }

    pub async fn shutdown_all(&self) {
        let mut connections = self.connections.lock().await;
        for (name, client) in connections.drain() {
            let _ = client.cancel().await;
            debug!("Shut down session HTTP connection: {}", name);
        }
    }
}
