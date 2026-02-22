use crate::agent::state::AgentSession;
use crate::agent::types::ToolCall;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

pub async fn execute_tool_calls(
    session_repo: Arc<dyn SessionRepository>,
    active_sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: Arc<MCPServiceProxyManager>,
    app_handle: AppHandle,
    session_id: String,
    tool_calls: Vec<ToolCall>,
) {
    for tool_call in tool_calls {
        let tool_name = tool_call.function.name.clone();
        let tool_call_id = tool_call.id.clone();
        let args_str = tool_call.function.arguments.clone();

        // Emit ToolExecutionStarted
        let event = crate::agent::events::AgentEvent::ToolExecutionStarted {
            session_id: session_id.clone(),
            tool_name: tool_name.clone(),
        };
        if let Err(e) = crate::agent::events::emit_agent_event(&app_handle, event) {
            log::error!("Failed to emit tool execution started event: {}", e);
        }

        // Parse arguments
        let args = match serde_json::from_str::<serde_json::Value>(&args_str) {
            Ok(v) => v,
            Err(e) => {
                log::error!("Failed to parse tool arguments: {}", e);
                let result = crate::commands::agent_commands::ToolExecutionResult {
                    success: false,
                    content: String::new(),
                    error: Some(format!("Failed to parse args: {}", e)),
                    is_error: true,
                    mcp_content: None,
                };
                // Handle result (error case)
                if let Err(e) = crate::agent::workflow::continue_workflow_after_tool(
                    &session_repo,
                    &active_sessions,
                    &proxy_manager,
                    &app_handle,
                    session_id.clone(),
                    tool_call_id,
                    result,
                )
                .await
                {
                    log::error!("Error continuing workflow after failed tool parse: {}", e);
                }
                continue; // Proceed to next tool
            }
        };

        // Call tool
        let result = match proxy_manager
            .call_tool(&session_id, &tool_name, args)
            .await
        {
            Ok(response) => {
                let mcp_content = crate::agent::tools::convert_mcp_response_content(
                    response.result.clone(),
                );

                // For logging/debugging only (not used in tool messages)
                let debug_content = response
                    .result
                    .as_ref()
                    .and_then(|r| serde_json::to_string_pretty(r).ok())
                    .unwrap_or_else(|| "{}".to_string());

                let is_error = response.error.is_some();
                let error_msg = response.error.map(|e| e.message);

                crate::commands::agent_commands::ToolExecutionResult {
                    success: !is_error,
                    content: debug_content,
                    error: error_msg,
                    is_error,
                    mcp_content,
                }
            }
            Err(e) => crate::commands::agent_commands::ToolExecutionResult {
                success: false,
                content: String::new(),
                error: Some(e),
                is_error: true,
                mcp_content: None,
            },
        };

        // Handle result and potentially continue workflow
        if let Err(e) = crate::agent::workflow::continue_workflow_after_tool(
            &session_repo,
            &active_sessions,
            &proxy_manager,
            &app_handle,
            session_id.clone(),
            tool_call_id,
            result,
        )
        .await
        {
            log::error!("Error continuing workflow after tool execution: {}", e);
        }
    }
}
