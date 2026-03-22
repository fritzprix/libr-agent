use crate::agent::state::AgentSession;
use crate::agent::types::ToolCall;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::{oneshot, RwLock};

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
        let requires_approval =
            crate::agent::tool_approvals::is_approval_required(&tool_name).await;

        let yolo_enabled = {
            let active = active_sessions.read().await;
            active
                .get(&session_id)
                .map(|s| s.yolo_mode.load(std::sync::atomic::Ordering::Relaxed))
                .unwrap_or(false)
        };

        if requires_approval && !yolo_enabled {
            let (tx, rx) = oneshot::channel();
            let attention_at = chrono::Utc::now().timestamp_millis();

            // Add tx to pending approvals
            {
                let active = active_sessions.read().await;
                if let Some(session) = active.get(&session_id) {
                    let mut approvals = session.pending_approvals.write().await;
                    approvals.insert(
                        tool_call_id.clone(),
                        crate::agent::state::PendingApprovalData {
                            sender: tx,
                            tool_name: tool_name.clone(),
                            arguments: args_str.clone(),
                        },
                    );
                }
            }

            if let Err(e) = session_repo
                .update_attention(
                    &session_id,
                    attention_at,
                    crate::repositories::session_repository::SessionAttentionReason::PendingApproval,
                )
                .await
            {
                log::error!(
                    "Failed to persist pending-approval attention for session {}: {}",
                    session_id,
                    e
                );
            }

            // Emit approval event
            let event = crate::agent::events::AgentEvent::ToolExecutionRequiresApproval {
                session_id: session_id.clone(),
                tool_call_id: tool_call_id.clone(),
                tool_name: tool_name.clone(),
                arguments: args_str.clone(),
            };
            if let Err(e) = crate::agent::events::emit_agent_event(&app_handle, event) {
                log::error!("Failed to emit ToolExecutionRequiresApproval event: {}", e);
            }

            // Wait for approval response
            match rx.await {
                Ok(approved) => {
                    if !approved {
                        // User rejected
                        let result = crate::commands::agent_commands::ToolExecutionResult {
                            success: false,
                            content: String::from("User rejected the tool execution."),
                            error: Some(String::from("User rejected the tool execution.")),
                            is_error: true,
                            mcp_content: None,
                        };

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
                            log::error!("Error continuing workflow after tool rejection: {}", e);
                        }
                        return; // Halt this loop, workflow continues normally handling rejection
                    }
                }
                Err(_) => {
                    log::warn!(
                        "Approval channel closed before receiving a response for {}",
                        tool_name
                    );
                    continue; // Skip execution if channel dropped
                }
            }
        }

        let result = match proxy_manager.call_tool(&session_id, &tool_name, args).await {
            Ok(response) => {
                // Derive is_error from both the JSON-RPC protocol error AND the
                // tool-level MCPResult.is_error / MCPContent error flag, so
                // builtin tool failures are not silently reported as success.
                let protocol_error = response.error.is_some();
                let tool_level_error = match &response.result {
                    Some(crate::mcp::types::MCPResponseResult::ToolCall(mcp_result)) => {
                        mcp_result.is_error == Some(true)
                            || mcp_result.content.as_ref().is_some_and(|content| {
                                content.iter().any(|c| {
                                    matches!(
                                        c,
                                        crate::mcp::types::MCPContent::Text {
                                            is_error: Some(true),
                                            ..
                                        }
                                    )
                                })
                            })
                    }
                    _ => false,
                };
                let is_error = protocol_error || tool_level_error;
                let error_msg = response.error.map(|e| e.message);

                // Avoid expensive pretty-serialization unless needed (no mcp_content present)
                // or when debug logging is enabled.
                let debug_content = if log::log_enabled!(log::Level::Debug) {
                    response
                        .result
                        .as_ref()
                        .and_then(|r| serde_json::to_string_pretty(r).ok())
                        .unwrap_or_else(|| "{}".to_string())
                } else {
                    String::new()
                };

                let mcp_content =
                    crate::agent::tools::convert_mcp_response_content(response.result);

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
