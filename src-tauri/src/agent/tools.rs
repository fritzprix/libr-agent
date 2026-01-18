use crate::agent::state::AgentSession;
use crate::commands::messages_commands::Message;
use crate::mcp::types::MCPContent;
use crate::mcp::MCPServiceProxyManager;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

/// Collect available tools for a session based on agent configuration
pub async fn collect_available_tools(
    session_id: &str,
    agent_config: &crate::agent::AgentConfig,
    proxy_manager: &Arc<MCPServiceProxyManager>,
) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
    let mut all_tools = Vec::new();

    // Get session proxy
    if let Some(proxy) = proxy_manager.get_proxy(session_id).await {
        // 1. Collect builtin tools (already filtered by extract_builtin_tool_ids during proxy creation)
        let builtin_tool_ids = proxy.builtin_tool_ids();

        log::debug!(
            "Session {} has {} builtin tool IDs configured",
            session_id,
            builtin_tool_ids.len()
        );

        // Get tools from each builtin server via the global MCP manager
        for tool_id in builtin_tool_ids {
            let server_tools = proxy.get_builtin_server_tools(&tool_id);
            log::debug!(
                "Builtin server '{}' provides {} tools",
                tool_id,
                server_tools.len()
            );
            all_tools.extend(server_tools);
        }

        log::info!(
            "Collected {} builtin tools for session {}",
            all_tools.len(),
            session_id
        );
    } else {
        log::warn!(
            "No proxy found for session {}, cannot collect builtin tools",
            session_id
        );
    }

    // 2. Collect external MCP tools (filtered by agent config)
    if !agent_config.mcp_server_ids.is_empty() {
        log::debug!(
            "Agent config allows {} external MCP servers",
            agent_config.mcp_server_ids.len()
        );

        // Get all external tools through public API
        let external_tools = proxy_manager
            .list_all_external_tools()
            .await
            .unwrap_or_default();

        // Filter by allowed server IDs
        // Tool names from external servers are formatted as: server_name__tool_name
        let filtered_external_tools: Vec<_> = external_tools
            .into_iter()
            .filter(|tool| {
                // Extract server name from tool name
                if let Some(server_name) = tool.name.split("__").next() {
                    agent_config
                        .mcp_server_ids
                        .contains(&server_name.to_string())
                } else {
                    false
                }
            })
            .collect();

        log::info!(
            "Collected {} external MCP tools (filtered by allowed servers) for session {}",
            filtered_external_tools.len(),
            session_id
        );

        all_tools.extend(filtered_external_tools);
    }

    log::info!(
        "Total tools available for session {}: {} tools",
        session_id,
        all_tools.len()
    );

    Ok(all_tools)
}

/// Extract builtin tool IDs from agent configuration
pub fn extract_builtin_tool_ids(agent_config: &crate::agent::AgentConfig) -> Vec<String> {
    let mut tool_ids = Vec::new();

    if let Some(allowed_aliases) = &agent_config.allowed_built_in_service_aliases {
        if allowed_aliases.is_empty() {
            return tool_ids;
        }

        for alias in allowed_aliases {
            match alias.as_str() {
                "bootstrap" => tool_ids.push("bootstrap".to_string()),
                "knowledge" => tool_ids.push("knowledge".to_string()),
                "planning" => tool_ids.push("planning".to_string()),
                "playbook" => tool_ids.push("playbook".to_string()),
                "assistant" => tool_ids.push("assistant".to_string()),
                "workspace" => tool_ids.push("workspace".to_string()),
                "content_store" | "contentstore" => tool_ids.push("content_store".to_string()),
                "ui" => tool_ids.push("ui".to_string()),
                "browser" => tool_ids.push("browser".to_string()),
                "mcp_manager" => tool_ids.push("mcp_manager".to_string()),
                _ => {
                    log::warn!("Unknown builtin service alias: {}", alias);
                }
            }
        }
    } else {
        // None = all builtin services allowed
        tool_ids.push("bootstrap".to_string());
        tool_ids.push("knowledge".to_string());
        tool_ids.push("planning".to_string());
        tool_ids.push("playbook".to_string());
        tool_ids.push("assistant".to_string());
        tool_ids.push("workspace".to_string());
        tool_ids.push("content_store".to_string());
        tool_ids.push("ui".to_string());
        tool_ids.push("browser".to_string());
        tool_ids.push("mcp_manager".to_string());
    }

    tool_ids
}

/// Create a tool result message from successful tool execution
pub fn create_tool_result_message(
    session_id: &str,
    tool_call_id: &str,
    content: String,
) -> Message {
    let now = chrono::Utc::now().timestamp_millis();
    let content_array = vec![MCPContent::Text { text: content }];

    Message {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "tool".to_string(),
        tool_call_id: Some(tool_call_id.to_string()),
        content: content_array,
        tool_calls: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        created_at: now,
        updated_at: now,
        source: Some("tool".to_string()),
        error: None,
    }
}

/// Create an error tool result message from failed tool execution
pub fn create_error_tool_result(
    session_id: &str,
    tool_call_id: &str,
    error_message: &str,
) -> Message {
    let now = chrono::Utc::now().timestamp_millis();
    let content_array = vec![MCPContent::Text {
        text: format!("Error: {}", error_message),
    }];

    Message {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "tool".to_string(),
        tool_call_id: Some(tool_call_id.to_string()),
        content: content_array,
        tool_calls: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        created_at: now,
        updated_at: now,
        source: Some("tool".to_string()),
        error: None,
    }
}

/// Convert MCP response result to agent MCPContent
pub fn convert_mcp_response_content(
    result: Option<crate::mcp::types::MCPResponseResult>,
) -> Option<Vec<crate::mcp::types::MCPContent>> {
    match result {
        Some(crate::mcp::types::MCPResponseResult::ToolCall(tool_result)) => tool_result.content,
        _ => None,
    }
}

/// Create a tool result message from strict MCP content
pub fn create_tool_result_message_with_content(
    session_id: &str,
    tool_call_id: &str,
    content: Vec<MCPContent>,
) -> Message {
    let now = chrono::Utc::now().timestamp_millis();

    Message {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        role: "tool".to_string(),
        tool_call_id: Some(tool_call_id.to_string()),
        content,
        tool_calls: None,
        is_streaming: Some(false),
        thinking: None,
        thinking_signature: None,
        assistant_id: None,
        attachments: None,
        tool_use: None,
        created_at: now,
        updated_at: now,
        source: Some("tool".to_string()),
        error: None,
    }
}

/// Handle tool execution result from frontend or internal execution
///
/// Returns `Ok(Some(messages))` if all pending tools for this turn have completed,
/// containing the accumulated tool results to be processed.
/// Returns `Ok(None)` if we are still waiting for other tools to complete.
pub async fn handle_tool_result(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    app_handle: &AppHandle,
    session_id: String,
    tool_call_id: String,
    result: crate::commands::agent_commands::ToolExecutionResult,
) -> Result<Option<Vec<Message>>, String> {
    // Check cancellation
    {
        let active = active_sessions.read().await;
        if let Some(session) = active.get(&session_id) {
            if session.cancellation_token.is_cancelled() {
                log::info!("Workflow cancelled for session: {}", session_id);
                return Err("Workflow was cancelled".to_string());
            }
        }
    }

    log::debug!(
        "Tool result received for session {}, tool_call_id: {}",
        session_id,
        tool_call_id
    );

    // Scope to hold the write lock
    {
        let mut active = active_sessions.write().await;
        if let Some(session) = active.get_mut(&session_id) {
            if let Some(pending) = &mut session.pending_execution {
                // Create Tool Message using helper methods
                let message = if result.is_error {
                    create_error_tool_result(
                        &session_id,
                        &tool_call_id,
                        result.error.as_deref().unwrap_or("Unknown error"),
                    )
                } else if let Some(mcp_content) = result.mcp_content {
                    create_tool_result_message_with_content(&session_id, &tool_call_id, mcp_content)
                } else {
                    create_tool_result_message(&session_id, &tool_call_id, result.content.clone())
                };

                pending.results.push(message);

                // Emit ToolExecutionCompleted event for external tools (progress tracking)
                if let Some(tool_name) = pending.tool_names.get(&tool_call_id) {
                    let event = crate::agent::events::AgentEvent::ToolExecutionCompleted {
                        session_id: session_id.clone(),
                        tool_name: tool_name.clone(),
                        success: !result.is_error,
                    };
                    let _ = crate::agent::events::emit_agent_event(app_handle, event);
                }

                log::debug!(
                    "Accumulated result {}/{} for session {}",
                    pending.results.len(),
                    pending.total_expected,
                    session_id
                );

                // Check if all results are in
                if pending.results.len() >= pending.total_expected {
                    // Move results out of pending state
                    let accumulated_messages: Vec<Message> = pending.results.drain(..).collect();
                    // Clear pending state
                    session.pending_execution = None;

                    // Return the accumulated messages
                    return Ok(Some(accumulated_messages));
                }
            } else {
                log::warn!(
                    "Received tool result for session {} but no pending execution state found",
                    session_id
                );
                return Ok(None); // Ignore or error? Safe to ignore to prevent crashes
            }
        } else {
            return Err(format!("Session not found: {}", session_id));
        }
    }

    // If we're here, it means we haven't finished collecting all results yet
    Ok(None)
}
