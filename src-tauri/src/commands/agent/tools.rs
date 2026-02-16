use crate::agent::AgentSessionManager;
use crate::mcp::types::ServiceContext;
use std::collections::HashMap;
use tauri::{command, State};

use super::types::{AgentResponse, ToolExecutionResult};

/// Handle tool execution result from frontend (called by ToolBridgeProvider in TS)
#[command]
pub async fn agent_handle_tool_result(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
    tool_call_id: String,
    result: ToolExecutionResult,
) -> Result<AgentResponse, String> {
    manager
        .handle_tool_result(session_id.clone(), tool_call_id, result)
        .await?;

    Ok(AgentResponse {
        success: true,
        message: format!("Tool result processed for session: {}", session_id),
        data: None,
    })
}

/// Call a builtin tool directly via proxy_manager (for testing and direct execution)
/// Returns the unwrapped MCPResult (not the full MCPResponse wrapper)
#[command]
pub async fn agent_call_builtin_tool(
    session_id: String,
    tool_name: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    use crate::mcp::types::MCPResponseResult;
    use crate::state::get_mcp_service_proxy_manager;

    let proxy_manager = get_mcp_service_proxy_manager();

    let response = proxy_manager
        .call_tool(&session_id, &tool_name, args)
        .await?;

    // Handle errors from tool execution
    if let Some(error) = response.error {
        return Err(format!("Tool execution error: {}", error.message));
    }

    // Extract result from MCPResponse
    let result = response
        .result
        .ok_or_else(|| "Tool execution returned no result or error".to_string())?;

    // Unwrap MCPResult from MCPResponseResult::ToolCall variant
    // This matches the TypeScript expectation of receiving MCPResult directly
    match result {
        MCPResponseResult::ToolCall(mcp_result) => {
            // Serialize MCPResult (with camelCase field names matching TypeScript interface)
            serde_json::to_value(mcp_result)
                .map_err(|e| format!("Failed to serialize MCPResult: {}", e))
        }
        _ => Err(format!(
            "Unexpected response type for builtin tool '{}': expected ToolCall variant",
            tool_name
        )),
    }
}

/// Get service contexts for a session
#[command]
pub async fn agent_get_service_contexts(
    session_id: String,
) -> Result<HashMap<String, ServiceContext>, String> {
    use crate::state::get_mcp_service_proxy_manager;

    let proxy_manager = get_mcp_service_proxy_manager();

    let proxy = proxy_manager
        .get_proxy(&session_id)
        .await
        .ok_or_else(|| format!("No proxy found for session: {}", session_id))?;

    Ok(proxy.get_service_contexts().await)
}

/// Get available tools for a specific agent session
/// Returns the filtered tool list based on agent configuration
/// This ensures UI displays the same tools that LLM can actually use
#[command]
pub async fn agent_get_available_tools(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
    manager.get_available_tools(&session_id).await
}

/// Get available tools for a session
#[command]
pub async fn agent_get_tools(
    manager: State<'_, AgentSessionManager>,
    session_id: String,
) -> Result<Vec<crate::mcp::types::MCPTool>, String> {
    manager.get_tools_for_session(&session_id).await
}
