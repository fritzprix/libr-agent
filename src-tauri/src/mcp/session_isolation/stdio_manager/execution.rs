use super::SessionMCPManager;
use crate::mcp::session_isolation::error::SessionMCPError;
use crate::mcp::types::{MCPError, MCPResponse, MCPResponseResult};
use log::{debug, error, info, warn};
use serde_json::Value;
use std::sync::atomic::Ordering;
use tokio_util::sync::CancellationToken;

impl SessionMCPManager {
    /// Calls a tool on the specified MCP server.
    ///
    /// This will spawn the process if it's not running, execute the tool call,
    /// and detect crashes.
    ///
    /// More specifically, this method:
    /// - ensures the stdio MCP process for `server_name` is running (spawning it
    ///   via `ensure_process_running` if needed),
    /// - increments an active call counter used for lifecycle and crash tracking,
    /// - registers a per-call [`CancellationToken`] so other APIs can cancel the
    ///   in-flight tool invocation,
    /// - forwards the request to the underlying `rmcp` client, and
    /// - inspects the result to surface transport/protocol failures or server
    ///   crashes as [`SessionMCPError`] variants.
    ///
    /// If the underlying MCP server crashes, hangs, or becomes unreachable while
    /// handling the request, the error is converted into an appropriate
    /// [`SessionMCPError`] so callers can react instead of silently succeeding.
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
                    .insert(server_name.to_string(), std::time::Instant::now());

                // Map rmcp Content to crate::mcp::types::MCPContent.
                // Since rmcp uses Annotated<RawContent>, we serialize through JSON and inspect
                // the "type" discriminator to reconstruct the appropriate local MCPContent variant.
                let local_content: Vec<crate::mcp::types::MCPContent> = call_result
                    .content
                    .into_iter()
                    .filter_map(|c| {
                        let json_val = serde_json::to_value(&c).ok()?;

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
                                    Some(crate::mcp::types::MCPContent::Image {
                                        data: Some(data),
                                        uri: None,
                                        mime_type,
                                    })
                                }
                                "resource" => {
                                    // Extract only the nested 'resource' field to avoid double-nesting
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

                let error_msg = format!("{}", e);
                // If the error indicates a connection/communication failure, remove the process
                // from the map so it will be respawned on next call
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
        self.ensure_process_running(server_name).await?;

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
}
