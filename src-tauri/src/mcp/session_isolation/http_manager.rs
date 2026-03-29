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

use crate::mcp::types::{
    ChannelServerMetadata, MCPResponse, MCPServerConfig, MCPTool, TransportConfig,
};

use super::error::SessionMCPError;

/// Type alias for HTTP MCP service connection
type HttpServiceConnection = Arc<RunningService<RoleClient, ()>>;

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
    connections: Arc<Mutex<HashMap<String, HttpServiceConnection>>>,

    /// Channel metadata discovered from initialize responses of external servers.
    channel_metadata: Arc<RwLock<HashMap<String, ChannelServerMetadata>>>,
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
            channel_metadata: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Starts a session-isolated HTTP connection for the given server
    pub async fn start_server(
        &self,
        server_name: &str,
        config: MCPServerConfig,
    ) -> anyhow::Result<()> {
        let (url, headers) = match &config.transport {
            TransportConfig::Http { url, headers, .. } => (url.clone(), headers.clone()),
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

        // NOTE: We intentionally DO NOT inject the libr-agent `session_id` as the HTTP header `Mcp-Session-Id`.
        // The RMCP transport specification requires the MCP server to dictate the session ID during
        // the initial SSE phase, and the internal `StreamableHttpClientTransport` module manages this automatically.
        // Injecting our internal app session ID (e.g. owsz7...) causes standard servers like GitHub Copilot
        // to reject the connection with a 400 Bad Request error.

        // Build reqwest client with fixed headers
        let client = reqwest::Client::builder()
            .default_headers(header_map)
            .build()
            .context("Failed to build HTTP client")?;

        let mut transport_config = StreamableHttpClientTransportConfig::with_uri(url);
        // Always allow stateless so servers that omit Mcp-Session-Id (e.g. exa) connect
        // successfully regardless of the enable_sse config flag.  Stateful servers that do
        // return a session ID still work correctly — allow_stateless only matters when the
        // server omits the header.  This matches the MCPServerManager (probe) path which
        // hardcodes allow_stateless = true.
        transport_config.allow_stateless = true;

        let transport = StreamableHttpClientTransport::with_client(client, transport_config);

        let mcp_client = ().serve(transport).await.map_err(|e| {
            error!("Failed to connect to HTTP MCP server {server_name}: {e}");
            anyhow!("HTTP connection failed: {e}")
        })?;

        self.update_channel_metadata(server_name, mcp_client.peer_info())
            .await;

        // Store connection
        {
            let mut connections = self.connections.lock().await;
            connections.insert(server_name.to_string(), Arc::new(mcp_client));
        }

        debug!(
            "Established HTTP connection for server: {} (Session: {})",
            server_name, self.session_id
        );

        // Auto-update tool cache in background (non-blocking)
        let connections = self.connections.clone();
        let server_name_clone = server_name.to_string();

        crate::mcp::service_proxy_manager::spawn_tool_cache_update(
            server_name.to_string(),
            self.session_id.clone(),
            "HTTP",
            move || async move {
                // Get the client from connections
                let client_opt = {
                    let conns = connections.lock().await;
                    conns.get(&server_name_clone).cloned()
                };

                match client_opt {
                    Some(client) => client
                        .list_all_tools()
                        .await
                        .map(|tools| {
                            tools
                                .into_iter()
                                .map(|tool| crate::mcp::types::MCPTool {
                                    name: tool.name.to_string(),
                                    title: None,
                                    description: tool.description.unwrap_or_default().to_string(),
                                    input_schema: crate::mcp::server_utils::convert_input_schema(
                                        serde_json::to_value(tool.input_schema).unwrap_or_else(
                                            |_| serde_json::Value::Object(serde_json::Map::new()),
                                        ),
                                    ),
                                    output_schema: None,
                                    annotations: None,
                                })
                                .collect()
                        })
                        .map_err(|e| e.to_string()),
                    None => Err("Connection not found".to_string()),
                }
            },
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
        // Bounded retry policy for "session expired" style failures.
        //
        // For streamable HTTP MCP, servers may invalidate sessions and return 404.
        // When that happens, we reconnect once (recreate the transport/client) and
        // retry the original tool call once.
        const MAX_SESSION_RETRIES: usize = 1;

        for attempt in 0..=MAX_SESSION_RETRIES {
            let result = self
                .call_tool_inner(server_name, tool_name, args.clone())
                .await;

            match result {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    let looks_like_session_expired =
                        Self::looks_like_session_expired_error(&e.to_string());

                    if looks_like_session_expired && attempt < MAX_SESSION_RETRIES {
                        warn!(
                            "HTTP MCP tool call failed with possible session expiration (attempt {}/{}). Reconnecting and retrying. Server: {}, Tool: {}. Error: {}",
                            attempt + 1,
                            MAX_SESSION_RETRIES + 1,
                            server_name,
                            tool_name,
                            e
                        );

                        // Drop existing connection and reconnect.
                        {
                            let mut connections = self.connections.lock().await;
                            connections.remove(server_name);
                        }

                        let config_opt =
                            { self.http_configs.read().await.get(server_name).cloned() };
                        if let Some(config) = config_opt {
                            if let Err(start_err) = self.start_server(server_name, config).await {
                                return Err(SessionMCPError::ExecutionError(format!(
                                    "Failed to reconnect HTTP server after session expiration: {start_err}"
                                )));
                            }
                        }

                        continue;
                    }

                    // No retry remaining, or not a session-expired failure.
                    return Err(e);
                }
            }
        }

        Err(SessionMCPError::ExecutionError(
            "HTTP MCP tool call failed after retry".to_string(),
        ))
    }

    fn looks_like_session_expired_error(message: &str) -> bool {
        let msg = message.to_lowercase();
        msg.contains("404")
            || msg.contains("session expired")
            || msg.contains("invalid session")
            || (msg.contains("not found") && msg.contains("session"))
    }

    async fn call_tool_inner(
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

    pub async fn list_channel_metadata(&self) -> Vec<ChannelServerMetadata> {
        self.channel_metadata
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    /// Check if a specific server is managed by this session.
    pub async fn has_server(&self, server_name: &str) -> bool {
        self.http_configs.read().await.contains_key(server_name)
    }

    async fn update_channel_metadata(
        &self,
        server_name: &str,
        peer_info: Option<&rmcp::model::ServerInfo>,
    ) {
        let mut metadata = self.channel_metadata.write().await;

        if let Some(channel) =
            crate::mcp::session_isolation::channel_metadata::extract_channel_server_metadata(
                server_name,
                peer_info,
            )
        {
            metadata.insert(server_name.to_string(), channel);
        } else {
            metadata.remove(server_name);
        }
    }

    pub async fn shutdown_all(&self) {
        let mut connections = self.connections.lock().await;
        for (name, client_arc) in connections.drain() {
            // Try to unwrap Arc, if it's the only reference we can cancel
            if let Ok(client) = Arc::try_unwrap(client_arc) {
                let _ = client.cancel().await;
                debug!("Shut down session HTTP connection: {}", name);
            } else {
                debug!(
                    "HTTP connection {} still has active references, skipping cancel",
                    name
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HttpSessionManager;

    #[test]
    fn test_looks_like_session_expired_error() {
        assert!(HttpSessionManager::looks_like_session_expired_error(
            "HTTP 404: Session not found"
        ));
        assert!(HttpSessionManager::looks_like_session_expired_error(
            "Invalid session id"
        ));
        assert!(HttpSessionManager::looks_like_session_expired_error(
            "session expired"
        ));

        assert!(!HttpSessionManager::looks_like_session_expired_error(
            "Tool call failed: connection refused"
        ));
        assert!(!HttpSessionManager::looks_like_session_expired_error(
            "Unauthorized"
        ));
    }
}
