use crate::agent::state::AgentSession;
use crate::agent::state::PendingApprovalKind;
use crate::agent::tool_approvals::{ToolApprovalRequest, ToolExecutionPolicyDecision};
use crate::agent::types::{ToolCall, ToolCallFunction};
use crate::commands::agent_commands::ToolExecutionResult;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::{oneshot, RwLock};

struct ToolExecutionContext<'a> {
    session_repo: &'a Arc<dyn SessionRepository>,
    active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &'a Arc<MCPServiceProxyManager>,
    app_handle: &'a AppHandle,
    session_id: &'a str,
}

enum ApprovalOutcome {
    Approved,
    Rejected,
    ChannelClosed,
}

impl ToolExecutionContext<'_> {
    fn emit_tool_execution_started(&self, tool_name: &str) {
        let event = crate::agent::events::AgentEvent::ToolExecutionStarted {
            session_id: self.session_id.to_string(),
            tool_name: tool_name.to_string(),
        };
        if let Err(error) = crate::agent::tauri_events::emit_agent_event(self.app_handle, event) {
            log::error!("Failed to emit tool execution started event: {}", error);
        }
    }

    async fn current_yolo_mode(&self) -> bool {
        let active = self.active_sessions.read().await;
        active
            .get(self.session_id)
            .map(|session| session.yolo_mode.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
    }

    async fn current_unsafe_mode(&self) -> bool {
        let active = self.active_sessions.read().await;
        active
            .get(self.session_id)
            .map(|session| {
                session
                    .unsafe_mode
                    .load(std::sync::atomic::Ordering::Relaxed)
            })
            .unwrap_or(false)
    }

    async fn continue_after_tool(
        &self,
        tool_call_id: &str,
        result: ToolExecutionResult,
        error_context: &str,
    ) {
        if let Err(error) = crate::agent::workflow::continue_workflow_after_tool(
            self.session_repo,
            self.active_sessions,
            self.proxy_manager,
            self.app_handle,
            self.session_id.to_string(),
            tool_call_id.to_string(),
            result,
        )
        .await
        {
            log::error!(
                "Error continuing workflow after {}: {}",
                error_context,
                error
            );
        }
    }

    async fn request_approval(
        &self,
        tool_call_id: &str,
        tool_name: &str,
        args_str: &str,
        approval_request: &ToolApprovalRequest,
        approval_kind: PendingApprovalKind,
    ) -> ApprovalOutcome {
        let (tx, rx) = oneshot::channel();
        let attention_at = chrono::Utc::now().timestamp_millis();
        let has_permission_relay_channel = self
            .proxy_manager
            .session_has_permission_relay_channels(self.session_id)
            .await;
        let request_id = has_permission_relay_channel
            .then(crate::agent::tool_approvals::generate_channel_permission_request_id);
        let description = Some(approval_request.description.clone());
        let input_preview = Some(approval_request.input_preview.clone());

        {
            let active = self.active_sessions.read().await;
            if let Some(session) = active.get(self.session_id) {
                let mut approvals = session.pending_approvals.write().await;
                approvals.insert(
                    tool_call_id.to_string(),
                    crate::agent::state::PendingApprovalData {
                        sender: tx,
                        tool_name: tool_name.to_string(),
                        arguments: args_str.to_string(),
                        approval_kind,
                        request_id: request_id.clone(),
                        description: description.clone(),
                        input_preview: input_preview.clone(),
                    },
                );
            }
        }

        if let Err(error) = self
            .session_repo
            .update_attention(
                self.session_id,
                attention_at,
                crate::repositories::session_repository::SessionAttentionReason::PendingApproval,
            )
            .await
        {
            log::error!(
                "Failed to persist pending-approval attention for session {}: {}",
                self.session_id,
                error
            );
        }

        let approval_event = crate::agent::events::AgentEvent::ToolExecutionRequiresApproval {
            session_id: self.session_id.to_string(),
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            arguments: args_str.to_string(),
            approval_kind,
            request_id: request_id.clone(),
            description: description.clone(),
            input_preview: input_preview.clone(),
        };
        if let Err(error) =
            crate::agent::tauri_events::emit_agent_event(self.app_handle, approval_event)
        {
            log::error!(
                "Failed to emit ToolExecutionRequiresApproval event: {}",
                error
            );
        }

        if let (Some(request_id), Some(description), Some(input_preview)) =
            (request_id, description, input_preview)
        {
            let channel_event = crate::agent::events::AgentEvent::ChannelPermissionRequest {
                session_id: self.session_id.to_string(),
                request_id,
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                approval_kind,
                description,
                input_preview,
            };
            if let Err(error) =
                crate::agent::tauri_events::emit_agent_event(self.app_handle, channel_event)
            {
                log::error!("Failed to emit ChannelPermissionRequest event: {}", error);
            }
        }

        match rx.await {
            Ok(true) => ApprovalOutcome::Approved,
            Ok(false) => ApprovalOutcome::Rejected,
            Err(_) => {
                log::warn!(
                    "Approval channel closed before receiving a response for {}",
                    tool_name
                );
                ApprovalOutcome::ChannelClosed
            }
        }
    }

    async fn execute_tool(&self, tool_name: &str, args: serde_json::Value) -> ToolExecutionResult {
        match self
            .proxy_manager
            .call_tool(self.session_id, tool_name, args)
            .await
        {
            Ok(response) => {
                let protocol_error = response.error.is_some();
                let tool_level_error = match &response.result {
                    Some(crate::mcp::types::MCPResponseResult::ToolCall(mcp_result)) => {
                        mcp_result.is_error == Some(true)
                            || mcp_result.content.as_ref().is_some_and(|content| {
                                content.iter().any(|content_item| {
                                    matches!(
                                        content_item,
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
                let error_msg = response.error.map(|error| error.message);
                let debug_content = if log::log_enabled!(log::Level::Debug) {
                    response
                        .result
                        .as_ref()
                        .and_then(|result| serde_json::to_string_pretty(result).ok())
                        .unwrap_or_else(|| "{}".to_string())
                } else {
                    String::new()
                };
                let mcp_content =
                    crate::agent::tools::convert_mcp_response_content(response.result.clone());
                let structured_content = match response.result {
                    Some(crate::mcp::types::MCPResponseResult::ToolCall(mcp_result)) => {
                        mcp_result.structured_content
                    }
                    _ => None,
                };

                ToolExecutionResult {
                    success: !is_error,
                    content: debug_content,
                    structured_content,
                    error: error_msg,
                    is_error,
                    mcp_content,
                }
            }
            Err(error) => ToolExecutionResult {
                success: false,
                content: String::new(),
                structured_content: None,
                error: Some(error),
                is_error: true,
                mcp_content: None,
            },
        }
    }
}

fn args_parse_error_result(error: &serde_json::Error) -> ToolExecutionResult {
    ToolExecutionResult {
        success: false,
        content: String::new(),
        structured_content: None,
        error: Some(format!("Failed to parse args: {}", error)),
        is_error: true,
        mcp_content: None,
    }
}

fn error_result(message: impl Into<String>) -> ToolExecutionResult {
    let message = message.into();
    ToolExecutionResult {
        success: false,
        content: message.clone(),
        structured_content: None,
        error: Some(message),
        is_error: true,
        mcp_content: None,
    }
}

pub async fn execute_tool_calls(
    session_repo: Arc<dyn SessionRepository>,
    active_sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: Arc<MCPServiceProxyManager>,
    app_handle: AppHandle,
    session_id: String,
    tool_calls: Vec<ToolCall>,
) {
    let context = ToolExecutionContext {
        session_repo: &session_repo,
        active_sessions: &active_sessions,
        proxy_manager: &proxy_manager,
        app_handle: &app_handle,
        session_id: &session_id,
    };

    for ToolCall {
        id: tool_call_id,
        function,
        ..
    } in tool_calls
    {
        let ToolCallFunction {
            name: tool_name,
            arguments: args_str,
        } = function;

        context.emit_tool_execution_started(&tool_name);

        let args = match serde_json::from_str::<serde_json::Value>(&args_str) {
            Ok(args) => args,
            Err(error) => {
                log::error!("Failed to parse tool arguments: {}", error);
                context
                    .continue_after_tool(
                        &tool_call_id,
                        args_parse_error_result(&error),
                        "failed tool parse",
                    )
                    .await;
                continue;
            }
        };

        let policy_decision =
            crate::agent::tool_approvals::evaluate_tool_execution_policy(&tool_name, &args).await;

        let unsafe_enabled = context.current_unsafe_mode().await;
        if let Some(blocked) = crate::agent::tool_approvals::blocked_execution_for_runtime(
            &policy_decision,
            unsafe_enabled,
        ) {
            context
                .continue_after_tool(
                    &tool_call_id,
                    error_result(blocked.message.clone()),
                    "policy-blocked tool execution",
                )
                .await;
            continue;
        }

        let yolo_enabled = context.current_yolo_mode().await;
        if let Some(approval_request) = crate::agent::tool_approvals::approval_request_for_runtime(
            &policy_decision,
            yolo_enabled,
            unsafe_enabled,
        ) {
            let approval_kind = match &policy_decision {
                ToolExecutionPolicyDecision::RequireHardApproval(_) => PendingApprovalKind::Hard,
                ToolExecutionPolicyDecision::RequireApproval(_) => PendingApprovalKind::Standard,
                _ => PendingApprovalKind::Standard,
            };
            match context
                .request_approval(
                    &tool_call_id,
                    &tool_name,
                    &args_str,
                    approval_request,
                    approval_kind,
                )
                .await
            {
                ApprovalOutcome::Approved => {}
                ApprovalOutcome::Rejected => {
                    context
                        .continue_after_tool(
                            &tool_call_id,
                            error_result("User rejected the tool execution."),
                            "tool rejection",
                        )
                        .await;
                    return;
                }
                ApprovalOutcome::ChannelClosed => continue,
            }
        }

        let result = context.execute_tool(&tool_name, args).await;
        context
            .continue_after_tool(&tool_call_id, result, "tool execution")
            .await;
    }
}
