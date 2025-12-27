use crate::mcp::server::MCPServerManager;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use thronglet::error::AgentError;
use thronglet::models::{Content, ToolDefinition, ToolResult};
use thronglet::traits::ToolProvider;

/// A wrapper struct to implement `thronglet::ToolProvider` for `MCPServerManager`.
/// `MCPServerManager` is already thread-safe (internally uses mutexes), so we can wrap it in Arc.
#[derive(Clone)]
pub struct WrappedMcpManager {
    pub inner: &'static MCPServerManager,
}

impl WrappedMcpManager {
    pub fn new(manager: &'static MCPServerManager) -> Self {
        Self { inner: manager }
    }
}

#[async_trait]
impl ToolProvider for WrappedMcpManager {
    async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: Value,
    ) -> Result<ToolResult, AgentError> {
        // Delegate to MCPServerManager::call_tool_unified
        // We set request_id to None to let it auto-generate, or we can propagate one if available.
        let response = self
            .inner
            .call_tool_unified(server_name, tool_name, args, None)
            .await;

        // Map MCPResponse to thronglet::ToolResult
        if let Some(error) = response.error {
            return Err(AgentError::ToolError(format!(
                "MCP Error {}: {} (Data: {:?})",
                error.code, error.message, error.data
            )));
        }

        if let Some(result_enum) = response.result {
            // Extract ToolCall from enum
            let mcp_result = match result_enum {
                crate::mcp::types::MCPResponseResult::ToolCall(result) => result,
                _ => {
                    return Err(AgentError::ToolError(
                        "Expected ToolCall result".to_string(),
                    ))
                }
            };

            let is_error = mcp_result.is_error.unwrap_or(false);

            let mut content_vec = Vec::new();
            if let Some(content_items) = &mcp_result.content {
                for item in content_items {
                    match item {
                        crate::mcp::types::MCPContent::Text { text } => {
                            content_vec.push(Content::Text { text: text.clone() });
                        }
                        crate::mcp::types::MCPContent::Resource { resource } => {
                            // Extract fields from resource Value
                            let uri = resource
                                .get("uri")
                                .and_then(|u| u.as_str())
                                .unwrap_or("")
                                .to_string();
                            let mime = resource
                                .get("mimeType")
                                .and_then(|m| m.as_str())
                                .unwrap_or("application/octet-stream")
                                .to_string();
                            let text = resource
                                .get("text")
                                .and_then(|t| t.as_str())
                                .map(|s| s.to_string());

                            content_vec.push(Content::Resource {
                                uri,
                                mime_type: mime,
                                text,
                            });
                        }
                    }
                }
            }

            // Handle case where result might be empty but successful
            if content_vec.is_empty() {
                content_vec.push(Content::Text {
                    text: "Tool executed successfully.".to_string(),
                });
            }

            Ok(ToolResult {
                tool_call_id: "".to_string(), // Caller will fill this info from the request context usually
                content: content_vec,
                is_error,
            })
        } else {
            // Null result but no error? quirky case.
            Ok(ToolResult {
                tool_call_id: "".to_string(),
                content: vec![Content::Text {
                    text: "Tool executed but returned no content.".to_string(),
                }],
                is_error: false,
            })
        }
    }

    async fn list_tools(
        &self,
        _server_names: Vec<String>,
    ) -> Result<Vec<ToolDefinition>, AgentError> {
        // Implement list tools logic using MCPServerManager::list_all_tools_unified
        let tools = self
            .inner
            .list_all_tools_unified()
            .await
            .map_err(|e| AgentError::ToolError(format!("Failed to list tools: {}", e)))?;

        let definitions = tools
            .into_iter()
            .map(|t| ToolDefinition {
                name: t.name,
                description: Some(t.description),
                input_schema: serde_json::to_value(t.input_schema).unwrap_or(Value::Null),
            })
            .collect();

        Ok(definitions)
    }
}
// State container for managing active agents
#[derive(Default)]
pub struct AgentRuntimeState {
    // Phase 1/2: Placeholder for agent registry
    // agents: Arc<Mutex<HashMap<String, Arc<Mutex<Agent>>>>>
}

#[tauri::command]
pub async fn agent_start(
    app_handle: tauri::AppHandle,
    _agent_state: tauri::State<'_, AgentRuntimeState>,
    pending_requests: tauri::State<'_, Arc<PendingLlmRequests>>,
    config: thronglet::models::AgentConfig,
) -> Result<String, String> {
    // 1. Get ToolProvider
    let mcp = crate::state::get_mcp_manager();
    let tool_provider = Arc::new(WrappedMcpManager::new(mcp));

    // 2. Create RemoteLLMProvider
    // AppHandle is cheap to clone
    let llm_provider = Arc::new(RemoteLLMProvider::new(
        app_handle.clone(),
        pending_requests.inner().clone(),
    ));

    // 3. Initialize Agent
    let agent = thronglet::agent::Agent::new(config, llm_provider, tool_provider);

    // 4. Spawn Agent Loop in Background
    let agent_id = uuid::Uuid::new_v4().to_string();
    let _agent_id_clone = agent_id.clone();

    // Suppress unused warn
    let _agent = agent;

    // In a real implementation, we'd store the agent handle in AgentRuntimeState
    // For now, we just let it run. We need an input mechanism though.
    // Thronglet Agent currently mimics a synchronous "run_loop" on input.
    // We need to decide how to feed input.
    //
    // For this Phase 3 integration, let's just Log that we started.
    // Real input mapping comes with `agent_input` command.

    // Store agent in state?
    // AgentRuntimeState needs to be upgraded to hold agents.
    // For now, let's just return success to prove instantiation works.

    log::info!("Agent {} initialized with RemoteLLMProvider", agent_id);

    Ok(agent_id)
}

// Phase 3: RemoteLLMProvider Implementation

use std::collections::HashMap;
use tauri::Emitter;
use thronglet::models::{LLMResponse, Message};
use thronglet::traits::LLMProvider;
use tokio::sync::oneshot;
use tokio::sync::Mutex;

/// Manages pending LLM requests that are sent to frontend waiting for response
#[derive(Default)]
pub struct PendingLlmRequests {
    pub requests: Mutex<HashMap<String, oneshot::Sender<Result<LLMResponse, AgentError>>>>,
}

#[derive(Clone)]
pub struct RemoteLLMProvider {
    app_handle: tauri::AppHandle,
    pending_requests: Arc<PendingLlmRequests>,
}

impl RemoteLLMProvider {
    pub fn new(app_handle: tauri::AppHandle, pending_requests: Arc<PendingLlmRequests>) -> Self {
        Self {
            app_handle,
            pending_requests,
        }
    }
}

#[async_trait]
impl LLMProvider for RemoteLLMProvider {
    async fn generate(
        &self,
        history: Vec<Message>,
        system_prompt: String,
    ) -> Result<LLMResponse, AgentError> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending_requests.requests.lock().await;
            pending.insert(request_id.clone(), tx);
        }

        // Emit event to Frontend
        let payload = serde_json::json!({
            "request_id": request_id,
            "messages": history,
            "system_prompt": system_prompt
        });

        if let Err(e) = self.app_handle.emit("agent://llm_request", payload) {
            return Err(AgentError::InternalError(format!(
                "Failed to emit llm_request: {}",
                e
            )));
        }

        // Wait for response via channel (will be fulfilled by agent_llm_response command)
        match rx.await {
            Ok(result) => result,
            Err(_) => Err(AgentError::InternalError(
                "LLM response channel closed unexpectedly".to_string(),
            )),
        }
    }
}

// Command called by Frontend to fulfill the request
#[tauri::command]
pub async fn agent_llm_response(
    state: tauri::State<'_, Arc<PendingLlmRequests>>,
    request_id: String,
    response: LLMResponse,
) -> Result<(), String> {
    let mut pending = state.requests.lock().await;

    if let Some(tx) = pending.remove(&request_id) {
        let _ = tx.send(Ok(response));
        Ok(())
    } else {
        Err(format!(
            "Request ID {} not found or already processed",
            request_id
        ))
    }
}
