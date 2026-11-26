use crate::agent::events::AgentEventDispatcher;
use crate::agent::state::{AgentSession, PendingApprovalKind};
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::agent::tool_approvals::ToolApprovalRequest;
use crate::commands::agent_commands::ToolExecutionResult;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::{oneshot, RwLock};

pub(super) struct ToolExecutionContext<'a> {
    pub(super) session_repo: &'a Arc<dyn SessionRepository>,
    pub(super) active_sessions: &'a Arc<RwLock<HashMap<String, AgentSession>>>,
    pub(super) proxy_manager: &'a Arc<MCPServiceProxyManager>,
    pub(super) app_handle: &'a AppHandle,
    pub(super) session_id: &'a str,
}

pub(super) enum ApprovalOutcome {
    Approved,
    Rejected,
    ChannelClosed,
}

/// Identifiers of the tool call awaiting approval: the session running it and the
/// call itself. Borrowed for the build; each payload copies what it needs.
pub(super) struct ToolCallRef<'a> {
    pub(super) session_id: &'a str,
    pub(super) tool_call_id: &'a str,
    pub(super) tool_name: &'a str,
}

/// The record to store and the event to emit for one pending approval. Both carry
/// the same display arguments: the request's masked arguments when it has them,
/// otherwise the tool call's own arguments. The approval card, the Session API
/// listing and the resume re-emit read these arguments; execution never does — it
/// runs the tool call's own arguments (#2968). Split out of `request_approval` so
/// the store-and-emit wiring is testable without a live session.
pub(super) fn build_pending_approval(
    call: &ToolCallRef<'_>,
    args_str: &str,
    approval_request: &ToolApprovalRequest,
    approval_kind: PendingApprovalKind,
    sender: oneshot::Sender<bool>,
    request_id: Option<String>,
) -> (
    crate::agent::state::PendingApprovalData,
    crate::agent::events::AgentEvent,
) {
    let display_arguments = approval_request
        .masked_arguments
        .as_deref()
        .unwrap_or(args_str);
    let description = Some(approval_request.description.clone());
    let input_preview = Some(approval_request.input_preview.clone());
    let data = crate::agent::state::PendingApprovalData {
        sender,
        tool_name: call.tool_name.to_string(),
        arguments: display_arguments.to_string(),
        approval_kind,
        request_id: request_id.clone(),
        description: description.clone(),
        input_preview: input_preview.clone(),
    };
    let event = crate::agent::events::AgentEvent::ToolExecutionRequiresApproval {
        session_id: call.session_id.to_string(),
        tool_call_id: call.tool_call_id.to_string(),
        tool_name: call.tool_name.to_string(),
        arguments: display_arguments.to_string(),
        approval_kind,
        request_id,
        description,
        input_preview,
    };
    (data, event)
}

impl ToolExecutionContext<'_> {
    pub(super) fn emit_tool_execution_started(&self, tool_name: &str) {
        let event = crate::agent::events::AgentEvent::ToolExecutionStarted {
            session_id: self.session_id.to_string(),
            tool_name: tool_name.to_string(),
        };
        if let Err(error) =
            TauriEventDispatcher::from_handle(self.app_handle).emit_agent_event(event)
        {
            log::error!("Failed to emit tool execution started event: {}", error);
        }
    }

    async fn current_execution_mode(&self) -> crate::execution_mode::ExecutionMode {
        let active = self.active_sessions.read().await;
        active
            .get(self.session_id)
            .map(|session| session.execution_mode())
            .unwrap_or(crate::execution_mode::ExecutionMode::Normal)
    }

    pub(super) async fn current_yolo_mode(&self) -> bool {
        matches!(
            self.current_execution_mode().await,
            crate::execution_mode::ExecutionMode::Yolo
        )
    }

    pub(super) async fn current_unsafe_mode(&self) -> bool {
        matches!(
            self.current_execution_mode().await,
            crate::execution_mode::ExecutionMode::Unsafe
        )
    }

    /// Best-effort lookup of the session's configured output-token budget.
    ///
    /// Used to phrase argument-size guidance relative to what the model can
    /// actually emit in one turn, instead of a hardcoded character count.
    pub(super) async fn configured_max_output_tokens(&self) -> Option<u32> {
        let metadata = {
            let active = self.active_sessions.read().await;
            active.get(self.session_id).map(|s| s.metadata.clone())
        }?;

        crate::agent::resolve_agent_config(&metadata)
            .await
            .ok()
            .and_then(|config| config.max_tokens)
    }

    pub(super) async fn continue_after_tool(
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
            if error == "Workflow was cancelled" || error.starts_with("Session not found") {
                log::info!(
                    "Workflow continuation skipped for session {} ({}): {}",
                    self.session_id,
                    error_context,
                    error
                );
            } else {
                log::error!(
                    "Error continuing workflow after {}: {}",
                    error_context,
                    error
                );
            }
        }
    }

    pub(super) async fn request_approval(
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
        // #2968: the card shows these arguments, so a transport that may carry
        // secrets is stored and emitted already masked; execution never reads
        // them back, it runs the tool call's own arguments.
        let (approval_data, approval_event) = build_pending_approval(
            &ToolCallRef {
                session_id: self.session_id,
                tool_call_id,
                tool_name,
            },
            args_str,
            approval_request,
            approval_kind,
            tx,
            request_id.clone(),
        );
        let description = approval_data.description.clone();
        let input_preview = approval_data.input_preview.clone();

        {
            let active = self.active_sessions.read().await;
            if let Some(session) = active.get(self.session_id) {
                let mut approvals = session.pending_approvals.write().await;
                approvals.insert(tool_call_id.to_string(), approval_data);
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

        if let Err(error) =
            TauriEventDispatcher::from_handle(self.app_handle).emit_agent_event(approval_event)
        {
            log::error!(
                "Failed to emit ToolExecutionRequiresApproval event: {}",
                error
            );
        }

        // Notification hook (#2925): run approved Notification handlers; ignore decisions.
        let mut notification_params = crate::services::hooks::HookEnvelopeParams::new(
            self.session_id,
            std::path::PathBuf::from("."),
            crate::services::hooks::HookEvent::Notification,
        );
        notification_params.notification_type = Some("permission".to_string());
        notification_params.notification_message = Some(format!(
            "Tool {} requires approval: {}",
            tool_name,
            description.as_deref().unwrap_or_default()
        ));
        let _ = crate::services::hooks::dispatch_hook_event(
            crate::services::hooks::HookEvent::Notification,
            "permission",
            &notification_params,
            self.session_id,
        )
        .await;

        if let (Some(request_id), Some(description), Some(input_preview)) =
            (request_id, description, input_preview)
        {
            let channel_event = crate::agent::events::AgentEvent::ChannelPermissionRequest {
                session_id: self.session_id.to_string(),
                request_id: request_id.clone(),
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
                approval_kind,
                description: description.clone(),
                input_preview: input_preview.clone(),
            };
            if let Err(error) =
                TauriEventDispatcher::from_handle(self.app_handle).emit_agent_event(channel_event)
            {
                log::error!("Failed to emit ChannelPermissionRequest event: {}", error);
            }

            if let Err(error) = self
                .proxy_manager
                .broadcast_channel_permission_request(
                    self.session_id,
                    crate::mcp::types::ChannelPermissionRequest {
                        request_id,
                        tool_name: tool_name.to_string(),
                        description,
                        input_preview,
                    },
                )
                .await
            {
                log::warn!(
                    "Failed to broadcast native channel permission request for session {}: {}",
                    self.session_id,
                    error
                );
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

    pub(super) async fn execute_tool(
        &self,
        tool_name: &str,
        args: serde_json::Value,
    ) -> ToolExecutionResult {
        match self
            .proxy_manager
            .call_tool(self.session_id, tool_name, args)
            .await
        {
            Ok(response) => {
                let protocol_error = response.error.is_some();
                let tool_level_error = match &response.result {
                    Some(crate::mcp::types::MCPResponseResult::ToolCall(mcp_result)) => {
                        mcp_result.indicates_error()
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
                    cancellation: None,
                    execution_duration_ms: None,
                }
            }
            Err(error) => ToolExecutionResult {
                success: false,
                content: String::new(),
                structured_content: None,
                error: Some(error),
                is_error: true,
                mcp_content: None,
                cancellation: None,
                execution_duration_ms: None,
            },
        }
    }
}
