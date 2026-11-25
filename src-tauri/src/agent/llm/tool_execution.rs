use crate::agent::events::AgentEventDispatcher;
use crate::agent::llm::natural_recovery::{
    build_loop_prevention_guidance, loop_prevention_tool_result, LoopPreventionShortCircuit,
};
use crate::agent::state::AgentSession;
use crate::agent::state::PendingApprovalKind;
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::agent::tool_approvals::{ToolApprovalRequest, ToolExecutionPolicyDecision};
use crate::agent::types::{ToolCall, ToolCallFunction};
use crate::commands::agent_commands::{ToolCancellation, ToolExecutionResult};
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::SessionRepository;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::{oneshot, RwLock};
use tokio_util::sync::CancellationToken;

/// Claude Code's `Notification` type string for a tool-approval prompt (#3005).
/// A plugin groups these with `"matcher": "permission_prompt"`.
pub(crate) const NOTIFICATION_PERMISSION_PROMPT: &str = "permission_prompt";

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

/// Identifiers of the tool call awaiting approval: the session running it and the
/// call itself. Borrowed for the build; each payload copies what it needs.
struct ToolCallRef<'a> {
    session_id: &'a str,
    tool_call_id: &'a str,
    tool_name: &'a str,
}

/// The record to store and the event to emit for one pending approval. Both carry
/// the same display arguments: the request's masked arguments when it has them,
/// otherwise the tool call's own arguments. The approval card, the Session API
/// listing and the resume re-emit read these arguments; execution never does — it
/// runs the tool call's own arguments (#2968). Split out of `request_approval` so
/// the store-and-emit wiring is testable without a live session.
fn build_pending_approval(
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
    fn emit_tool_execution_started(&self, tool_name: &str) {
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

    async fn current_yolo_mode(&self) -> bool {
        matches!(
            self.current_execution_mode().await,
            crate::execution_mode::ExecutionMode::Yolo
        )
    }

    async fn current_unsafe_mode(&self) -> bool {
        matches!(
            self.current_execution_mode().await,
            crate::execution_mode::ExecutionMode::Unsafe
        )
    }

    /// Best-effort lookup of the session's configured output-token budget.
    ///
    /// Used to phrase argument-size guidance relative to what the model can
    /// actually emit in one turn, instead of a hardcoded character count.
    async fn configured_max_output_tokens(&self) -> Option<u32> {
        let metadata = {
            let active = self.active_sessions.read().await;
            active.get(self.session_id).map(|s| s.metadata.clone())
        }?;

        crate::agent::resolve_agent_config(&metadata)
            .await
            .ok()
            .and_then(|config| config.max_tokens)
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

        // Notification hook (#2925, #3005): run approved Notification handlers;
        // ignore decisions. The type string is the Claude Code one for a tool
        // approval prompt (`permission_prompt`), so a plugin's
        // `"matcher": "permission_prompt"` group matches.
        let mut notification_params = crate::services::hooks::HookEnvelopeParams::new(
            self.session_id,
            std::path::PathBuf::from("."),
            crate::services::hooks::HookEvent::Notification,
        );
        notification_params.notification_type = Some(NOTIFICATION_PERMISSION_PROMPT.to_string());
        notification_params.notification_message = Some(format!(
            "Tool {} requires approval: {}",
            tool_name,
            description.as_deref().unwrap_or_default()
        ));
        let _ = crate::services::hooks::dispatch_hook_event(
            crate::services::hooks::HookEvent::Notification,
            NOTIFICATION_PERMISSION_PROMPT,
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

use crate::agent::llm::tool_args_validation::{
    args_preview, inspect_tool_call_arguments, ArgsParseFailureKind,
};

/// Build a guided tool-result for malformed / truncated tool-call arguments so the
/// agent can recover on the next turn instead of retrying the same broken payload.
///
/// This intentionally closes the tool call with an error result (via
/// `continue_after_tool`). The existing loop-prevention circuit then treats
/// repeated identical (tool, args) + error outcomes as a streak (default soft
/// block at 3).
fn args_parse_error_result(
    tool_name: &str,
    args_str: &str,
    kind: ArgsParseFailureKind,
    parse_error: &str,
    max_output_tokens: Option<u32>,
) -> ToolExecutionResult {
    let args_bytes = crate::agent::llm::tool_args_validation::parse_redacted_byte_length(args_str)
        .unwrap_or(args_str.len());

    let budget_info = match max_output_tokens {
        Some(tokens) => format!(" (max output tokens budget: ~{})", tokens),
        None => String::new(),
    };

    let (headline, mut guidance) = if kind.is_truncated() {
        (
            format!(
                "Failed to parse arguments for `{tool_name}`: output budget exceeded{budget_info}, arguments truncated ({parse_error}). The tool call was not executed."
            ),
            vec![
                "Output budget exceeded: arguments were truncated before JSON was complete".to_string(),
                "The tool call was not executed".to_string(),
                "Split the work into smaller tool calls with smaller payloads".to_string(),
                "Do NOT retry the identical oversized payload — it will truncate again".to_string(),
            ],
        )
    } else {
        (
            format!(
                "Failed to parse arguments for `{tool_name}`: invalid JSON ({parse_error}). The tool call was not executed."
            ),
            vec![
                "Fix the JSON syntax (quotes, commas, braces, escapes) and call the tool again".to_string(),
                "The tool call was not executed".to_string(),
                "If argument payloads are large, split the work into smaller tool calls".to_string(),
            ],
        )
    };

    if let Some(tokens) = max_output_tokens {
        guidance.push(format!(
            "Session output budget is ~{} tokens; keep each tool call JSON well below this budget.",
            tokens
        ));
    }

    guidance.push(
        "Repeating the same broken arguments will eventually be blocked by loop prevention"
            .to_string(),
    );

    let guidance_text = guidance
        .iter()
        .enumerate()
        .map(|(i, step)| format!("{}. {}", i + 1, step))
        .collect::<Vec<_>>()
        .join("\n");
    let message = format!(
        "✗ {headline}\n\n{}\n{guidance_text}",
        crate::mcp::builtin::error_guidance::hint_headers::ERROR_RECOVERY
    );

    let structured = serde_json::json!({
        "errorKind": kind.as_error_kind(),
        "toolName": tool_name,
        "parseError": parse_error,
        "argsByteLength": args_bytes,
        "likelyTruncated": kind.is_truncated(),
        "maxOutputTokens": max_output_tokens,
        "argsPreview": args_preview(args_str, 240),
        "nextActions": guidance,
        "recoverable": true,
    });

    ToolExecutionResult {
        success: false,
        content: message.clone(),
        mcp_content: Some(vec![crate::mcp::types::MCPContent::Text {
            text: message.clone(),
        }]),
        structured_content: Some(structured),
        error: Some(message),
        is_error: true,
        cancellation: None,
        execution_duration_ms: None,
    }
}

fn tool_result_from_mcp_result(result: crate::mcp::types::MCPResult) -> ToolExecutionResult {
    let is_error = result.indicates_error();
    let structured_content = result.structured_content.clone();
    ToolExecutionResult {
        success: !is_error,
        content: String::new(),
        structured_content,
        error: None,
        is_error,
        mcp_content: crate::agent::tools::convert_mcp_response_content(Some(
            crate::mcp::types::MCPResponseResult::ToolCall(result),
        )),
        cancellation: None,
        execution_duration_ms: None,
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
        cancellation: None,
        execution_duration_ms: None,
    }
}

fn cancelled_tool_result() -> ToolExecutionResult {
    let mut result = error_result("Tool call cancelled by user.");
    result.cancellation = Some(ToolCancellation::user());
    result
}

async fn session_cancel_requested(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    cancellation_token: &CancellationToken,
) -> bool {
    if cancellation_token.is_cancelled() {
        return true;
    }
    let active = active_sessions.read().await;
    active
        .get(session_id)
        .map(|session| session.cancel_pending.load(Ordering::SeqCst))
        .unwrap_or(false)
}

async fn inject_cancel_tombstones_for_remaining(
    context: &ToolExecutionContext<'_>,
    remaining: &[ToolCall],
) {
    for tool_call in remaining {
        log::info!(
            "Injecting cancel tombstone for tool call {} in session {}",
            tool_call.id,
            context.session_id
        );
        context
            .continue_after_tool(
                &tool_call.id,
                cancelled_tool_result(),
                "cancelled tool call",
            )
            .await;
    }
}

pub async fn execute_tool_calls(
    session_repo: Arc<dyn SessionRepository>,
    active_sessions: Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: Arc<MCPServiceProxyManager>,
    app_handle: AppHandle,
    session_id: String,
    tool_calls: Vec<ToolCall>,
    loop_prevention_short_circuits: HashMap<String, LoopPreventionShortCircuit>,
) {
    let cancellation_token = {
        let active = active_sessions.read().await;
        match active.get(&session_id) {
            Some(session) => session.cancellation_token.clone(),
            None => {
                log::warn!(
                    "execute_tool_calls: session {} not found; skipping tool batch",
                    session_id
                );
                return;
            }
        }
    };

    let context = ToolExecutionContext {
        session_repo: &session_repo,
        active_sessions: &active_sessions,
        proxy_manager: &proxy_manager,
        app_handle: &app_handle,
        session_id: &session_id,
    };

    // Index-based loop (not `for tool_calls`) so a mid-batch cancel can
    // tombstone `tool_calls[index..]` and break without consuming the iterator.
    // Every continue/success path must bump `index` before the next check.
    let mut index = 0;
    while index < tool_calls.len() {
        if session_cancel_requested(&active_sessions, &session_id, &cancellation_token).await {
            inject_cancel_tombstones_for_remaining(&context, &tool_calls[index..]).await;
            break;
        }

        let ToolCall {
            id: tool_call_id,
            function,
            ..
        } = &tool_calls[index];
        let ToolCallFunction {
            name: tool_name,
            arguments: args_str,
        } = function;

        context.emit_tool_execution_started(tool_name);

        if let Some(short_circuit) = loop_prevention_short_circuits.get(tool_call_id) {
            let guidance = build_loop_prevention_guidance(short_circuit);
            context
                .continue_after_tool(
                    tool_call_id,
                    loop_prevention_tool_result(&guidance),
                    "loop prevention short-circuit",
                )
                .await;
            index += 1;
            continue;
        }

        let args = match inspect_tool_call_arguments(args_str) {
            Ok(map) => serde_json::Value::Object(map),
            Err((kind, parse_error)) => {
                log::error!(
                    "Failed to parse tool arguments for {}: {} ({})",
                    tool_name,
                    parse_error,
                    kind.as_error_kind()
                );
                let max_output_tokens = context.configured_max_output_tokens().await;
                context
                    .continue_after_tool(
                        tool_call_id,
                        args_parse_error_result(
                            tool_name,
                            args_str,
                            kind,
                            &parse_error,
                            max_output_tokens,
                        ),
                        "failed tool parse",
                    )
                    .await;
                index += 1;
                continue;
            }
        };

        let unsafe_enabled = context.current_unsafe_mode().await;
        let mut guards = pre_execution_guards(
            &active_sessions,
            &session_id,
            unsafe_enabled,
            tool_name,
            &args,
        )
        .await;
        if let Some((refusal, reason)) = guards.refusal.take() {
            context
                .continue_after_tool(tool_call_id, refusal, reason)
                .await;
            index += 1;
            continue;
        }

        // PreToolUse hook (#2925): runs after hard security/root guards and before
        // approval/execution. Applies matcher on `tool_name` and `permissionDecision`
        // (allow / deny / ask).
        let mut pre_tool_params = crate::services::hooks::HookEnvelopeParams::new(
            &session_id,
            std::path::PathBuf::from("."),
            crate::services::hooks::HookEvent::PreToolUse,
        );
        pre_tool_params.tool_name = Some(tool_name.clone());
        pre_tool_params.tool_input = Some(args.clone());
        let pre_tool_outcome = crate::services::hooks::dispatch_hook_event(
            crate::services::hooks::HookEvent::PreToolUse,
            tool_name,
            &pre_tool_params,
            &session_id,
        )
        .await;
        if let Some(ctx) = pre_tool_outcome.joined_additional_context() {
            crate::services::hooks::push_turn_hook_context(&session_id, &ctx);
        }

        // `updatedInput` (#3005): apply the hook's rewrite through the
        // production step `apply_hook_input_rewrite` — object-only, Block
        // drops it, and the rewritten arguments re-run every guard.
        let rewrite = apply_hook_input_rewrite(
            &active_sessions,
            &session_id,
            unsafe_enabled,
            tool_name,
            &pre_tool_outcome,
            args_str,
            guards,
        )
        .await;
        let (args, guards, effective_args_json) = match rewrite {
            HookInputRewrite::Unchanged(guards) => (args, guards, args_str.clone()),
            HookInputRewrite::Refused((refusal, reason)) => {
                context
                    .continue_after_tool(tool_call_id, refusal, reason)
                    .await;
                index += 1;
                continue;
            }
            HookInputRewrite::Applied(effective) => {
                (effective.args, effective.guards, effective.args_json)
            }
        };
        let args_str = effective_args_json.as_str();
        let policy_decision = guards.policy_decision;

        let yolo_enabled = context.current_yolo_mode().await;
        let is_hard_approval = matches!(
            &policy_decision,
            ToolExecutionPolicyDecision::RequireHardApproval(_)
        );
        let base_approval_request = crate::agent::tool_approvals::approval_request_for_runtime(
            &policy_decision,
            yolo_enabled,
            unsafe_enabled,
        )
        .cloned();

        match apply_pre_tool_hook_decision(
            tool_name,
            args_str,
            base_approval_request,
            is_hard_approval,
            pre_tool_outcome.decision,
            pre_tool_outcome.reason,
        ) {
            PreToolHookResolution::Block { reason } => {
                context
                    .continue_after_tool(
                        tool_call_id,
                        error_result(reason),
                        "hook-blocked tool execution",
                    )
                    .await;
                index += 1;
                continue;
            }
            PreToolHookResolution::Execute => {}
            PreToolHookResolution::RequestApproval {
                request: approval_request,
                kind: approval_kind,
            } => {
                match context
                    .request_approval(
                        tool_call_id,
                        tool_name,
                        args_str,
                        &approval_request,
                        approval_kind,
                    )
                    .await
                {
                    ApprovalOutcome::Approved => {}
                    ApprovalOutcome::Rejected => {
                        context
                            .continue_after_tool(
                                tool_call_id,
                                error_result("User rejected the tool execution."),
                                "tool rejection",
                            )
                            .await;
                        // User rejected this tool — cancel remaining calls in the batch too.
                        inject_cancel_tombstones_for_remaining(&context, &tool_calls[index + 1..])
                            .await;
                        break;
                    }
                    ApprovalOutcome::ChannelClosed => {
                        let session_status = {
                            let active = context.active_sessions.read().await;
                            active
                                .get(context.session_id)
                                .map(|s| s.metadata.status.clone())
                        };
                        if !crate::agent::workflow::should_continue_after_approval_channel_closed(
                            session_status.as_ref(),
                        ) {
                            log::info!(
                                "Session {} is no longer active and busy ({:?}) after approval channel closure; cleanly discarding workflow continuation",
                                context.session_id,
                                session_status
                            );
                            break;
                        }

                        // Cancel/terminate dropped the approval waiter while the session is still active and busy.
                        // Close this call and let the cancel check at the top tombstone any remainder.
                        context
                            .continue_after_tool(
                                tool_call_id,
                                cancelled_tool_result(),
                                "approval cancelled",
                            )
                            .await;
                        index += 1;
                        continue;
                    }
                }
            }
        }

        // `args` is moved into `execute_tool` below; keep a copy for the
        // PostToolUse hook envelope.
        let post_tool_input = args.clone();
        let execution_started = std::time::Instant::now();
        let (mut result, process_was_cancelled) = proxy_manager
            .run_with_process_cancel_scope(
                &session_id,
                crate::agent::state::with_tool_call_id(
                    tool_call_id.clone(),
                    context.execute_tool(tool_name, args),
                ),
            )
            .await;
        result.execution_duration_ms = Some(execution_started.elapsed().as_millis() as u64);
        if result.is_error
            && (process_was_cancelled
                || session_cancel_requested(&active_sessions, &session_id, &cancellation_token)
                    .await)
        {
            result.cancellation = Some(ToolCancellation::user());
        }
        let tool_ended_with_error = result.is_error;
        context
            .continue_after_tool(tool_call_id, result, "tool execution")
            .await;

        // PostToolUse hook (#2925): runs after a successful tool call. Adds
        // `additionalContext` and `decision: "block"` feedback to the next LLM request.
        if !tool_ended_with_error {
            let mut post_tool_params = crate::services::hooks::HookEnvelopeParams::new(
                &session_id,
                std::path::PathBuf::from("."),
                crate::services::hooks::HookEvent::PostToolUse,
            );
            post_tool_params.tool_name = Some(tool_name.clone());
            post_tool_params.tool_input = Some(post_tool_input);
            post_tool_params.tool_response = Some(serde_json::json!({ "is_error": false }));
            let post_tool_outcome = crate::services::hooks::dispatch_hook_event(
                crate::services::hooks::HookEvent::PostToolUse,
                tool_name,
                &post_tool_params,
                &session_id,
            )
            .await;
            if let Some(ctx) = post_tool_outcome.joined_additional_context() {
                crate::services::hooks::push_turn_hook_context(&session_id, &ctx);
            }
            if post_tool_outcome.decision == Some(crate::services::hooks::HookDecision::Block) {
                let reason = post_tool_outcome
                    .reason
                    .filter(|r| !r.trim().is_empty())
                    .unwrap_or_else(|| {
                        format!(
                            "PostToolUse hook blocked continuation after '{}'.",
                            tool_name
                        )
                    });
                crate::services::hooks::push_turn_hook_context(
                    &session_id,
                    &format!("[PostToolUse hook block]: {reason}"),
                );
            }
        }
        index += 1;
    }
}

/// Pre-execution guards for one argument object (#3005).
struct PreExecutionGuards {
    policy_decision: ToolExecutionPolicyDecision,
    /// The result to return instead of executing, plus its error context.
    refusal: Option<(ToolExecutionResult, &'static str)>,
}

/// Policy block and root-execution refusal for `args`.
///
/// A `PreToolUse` hook may rewrite the arguments (`updatedInput`), so the same
/// guards run again on the rewritten arguments: a hook rewrite must not walk
/// around the policy block or the host root refusal.
async fn pre_execution_guards(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    unsafe_enabled: bool,
    tool_name: &str,
    args: &serde_json::Value,
) -> PreExecutionGuards {
    let policy_decision =
        crate::agent::tool_approvals::evaluate_tool_execution_policy(tool_name, args).await;

    // Build the refusal before moving `policy_decision`: the borrow inside
    // `blocked_execution_for_runtime` ends with this statement.
    let blocked_refusal = crate::agent::tool_approvals::blocked_execution_for_runtime(
        &policy_decision,
        unsafe_enabled,
    )
    .map(|blocked| error_result(blocked.message.clone()));
    if let Some(refusal) = blocked_refusal {
        return PreExecutionGuards {
            policy_decision,
            refusal: Some((refusal, "policy-blocked tool execution")),
        };
    }

    // Only the in-memory metadata is read: a session that is not loaded gets the host refusal.
    let root_refusal = {
        let active = active_sessions.read().await;
        crate::agent::tool_approvals::root_execution_refusal(
            tool_name,
            args,
            active.get(session_id).map(|session| &session.metadata),
        )
    };
    PreExecutionGuards {
        policy_decision,
        refusal: root_refusal.map(|refusal| {
            (
                tool_result_from_mcp_result(refusal),
                "root execution unavailable",
            )
        }),
    }
}

/// The effective call inputs after the `PreToolUse` `updatedInput` step
/// (#3005): what the guards checked, what the approval card shows, and what
/// the tool receives.
struct EffectiveCall {
    args: serde_json::Value,
    args_json: String,
    guards: PreExecutionGuards,
}

/// Outcome of the `updatedInput` step of the tool loop. Every non-`Applied`
/// outcome carries the original guards back so the loop keeps them.
enum HookInputRewrite {
    /// No usable rewrite: the model's arguments and their guards stand.
    Unchanged(PreExecutionGuards),
    /// The rewrite was applied and passed the guards; these are the arguments
    /// the tool receives and the approval card shows.
    Applied(EffectiveCall),
    /// The rewrite was applied but the re-run guards refused it: the tool
    /// must not run, and this result is returned instead.
    Refused((ToolExecutionResult, &'static str)),
}

/// One step of the tool loop (#3005, gatekeeper G1.4/G1.5): apply the hook's
/// `updatedInput`. Only a JSON object may replace the argument object — a
/// string, number, or array would make every key-reading guard see nothing,
/// so anything else is ignored with a visible warning and the original
/// arguments stand. A `Block` decision ignores any rewrite: the call never
/// runs. A usable rewrite re-runs the same policy and root guards, so a hook
/// cannot walk a command around them that the model could not run.
async fn apply_hook_input_rewrite(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    unsafe_enabled: bool,
    tool_name: &str,
    outcome: &crate::services::hooks::HookDispatchResult,
    original_args_json: &str,
    original_guards: PreExecutionGuards,
) -> HookInputRewrite {
    let rewrite = outcome
        .updated_input
        .clone()
        .filter(|_| outcome.decision != Some(crate::services::hooks::HookDecision::Block));
    let Some(rewritten) = rewrite else {
        return HookInputRewrite::Unchanged(original_guards);
    };
    // Non-object rewrites are already rejected at parse time in
    // `hooks/output.rs`, so `updated_input` is object-or-none by contract
    // (#3005 G2.3): no second check here.
    let args_json =
        serde_json::to_string(&rewritten).unwrap_or_else(|_| original_args_json.to_string());
    let guards = pre_execution_guards(
        active_sessions,
        session_id,
        unsafe_enabled,
        tool_name,
        &rewritten,
    )
    .await;
    match guards.refusal {
        Some(refusal) => HookInputRewrite::Refused(refusal),
        None => HookInputRewrite::Applied(EffectiveCall {
            args: rewritten,
            args_json,
            guards,
        }),
    }
}

/// Resolution of combining runtime tool-approval policy with a `PreToolUse` hook
/// decision (#2925 G1.2). Pure so every policy × hook-decision cell is unit-tested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PreToolHookResolution {
    Execute,
    RequestApproval {
        request: crate::agent::tool_approvals::ToolApprovalRequest,
        kind: PendingApprovalKind,
    },
    Block {
        reason: String,
    },
}

pub(crate) fn apply_pre_tool_hook_decision(
    tool_name: &str,
    args_str: &str,
    approval_request: Option<crate::agent::tool_approvals::ToolApprovalRequest>,
    is_hard_approval: bool,
    decision: Option<crate::services::hooks::HookDecision>,
    reason: Option<String>,
) -> PreToolHookResolution {
    let trimmed_reason = reason.filter(|r| !r.trim().is_empty());
    match decision {
        Some(crate::services::hooks::HookDecision::Block) => PreToolHookResolution::Block {
            reason: trimmed_reason
                .unwrap_or_else(|| format!("Tool '{}' blocked by PreToolUse hook.", tool_name)),
        },
        Some(crate::services::hooks::HookDecision::Allow) if !is_hard_approval => {
            PreToolHookResolution::Execute
        }
        Some(crate::services::hooks::HookDecision::Ask) => {
            let kind = if is_hard_approval {
                PendingApprovalKind::Hard
            } else {
                PendingApprovalKind::Standard
            };
            let request = approval_request.unwrap_or_else(|| {
                crate::agent::tool_approvals::ToolApprovalRequest {
                    description: trimmed_reason.unwrap_or_else(|| {
                        format!("PreToolUse hook requested confirmation for {tool_name}")
                    }),
                    input_preview: args_str.to_string(),
                    masked_arguments: None,
                }
            });
            PreToolHookResolution::RequestApproval { request, kind }
        }
        _ => match approval_request {
            Some(request) => PreToolHookResolution::RequestApproval {
                request,
                kind: if is_hard_approval {
                    PendingApprovalKind::Hard
                } else {
                    PendingApprovalKind::Standard
                },
            },
            None => PreToolHookResolution::Execute,
        },
    }
}

#[cfg(test)]
mod pre_tool_hook_tests {
    use super::{apply_pre_tool_hook_decision, PreToolHookResolution};
    use crate::agent::state::PendingApprovalKind;
    use crate::agent::tool_approvals::ToolApprovalRequest;
    use crate::services::hooks::HookDecision;

    fn sample_req() -> ToolApprovalRequest {
        ToolApprovalRequest {
            description: "policy requires approval".to_string(),
            input_preview: "{\"cmd\":\"rm\"}".to_string(),
            masked_arguments: None,
        }
    }

    #[test]
    fn block_always_wins_and_propagates_reason() {
        for (req, hard) in [
            (None, false),
            (Some(sample_req()), false),
            (Some(sample_req()), true),
        ] {
            let res = apply_pre_tool_hook_decision(
                "workspace__runShell",
                "{}",
                req,
                hard,
                Some(HookDecision::Block),
                Some("denied by policy hook".to_string()),
            );
            assert_eq!(
                res,
                PreToolHookResolution::Block {
                    reason: "denied by policy hook".to_string()
                }
            );
        }

        let fallback = apply_pre_tool_hook_decision(
            "workspace__runShell",
            "{}",
            None,
            false,
            Some(HookDecision::Block),
            Some("   ".to_string()),
        );
        assert_eq!(
            fallback,
            PreToolHookResolution::Block {
                reason: "Tool 'workspace__runShell' blocked by PreToolUse hook.".to_string()
            }
        );
    }

    #[test]
    fn allow_clears_standard_approval_but_never_clears_hard_approval() {
        let standard = apply_pre_tool_hook_decision(
            "workspace__writeFile",
            "{}",
            Some(sample_req()),
            false,
            Some(HookDecision::Allow),
            None,
        );
        assert_eq!(standard, PreToolHookResolution::Execute);

        let hard = apply_pre_tool_hook_decision(
            "workspace__runShell",
            "{}",
            Some(sample_req()),
            true,
            Some(HookDecision::Allow),
            None,
        );
        assert_eq!(
            hard,
            PreToolHookResolution::RequestApproval {
                request: sample_req(),
                kind: PendingApprovalKind::Hard,
            }
        );
    }

    #[test]
    fn ask_forces_confirmation_even_when_runtime_had_no_approval_request() {
        // Simulates YOLO mode where `approval_request_for_runtime` returned None.
        let yolo_ask = apply_pre_tool_hook_decision(
            "workspace__runShell",
            "{\"cmd\":\"ls\"}",
            None,
            false,
            Some(HookDecision::Ask),
            Some("confirm shell command".to_string()),
        );
        assert_eq!(
            yolo_ask,
            PreToolHookResolution::RequestApproval {
                request: ToolApprovalRequest {
                    description: "confirm shell command".to_string(),
                    input_preview: "{\"cmd\":\"ls\"}".to_string(),
                    masked_arguments: None,
                },
                kind: PendingApprovalKind::Standard,
            }
        );

        let hard_ask = apply_pre_tool_hook_decision(
            "workspace__runShell",
            "{}",
            Some(sample_req()),
            true,
            Some(HookDecision::Ask),
            None,
        );
        assert_eq!(
            hard_ask,
            PreToolHookResolution::RequestApproval {
                request: sample_req(),
                kind: PendingApprovalKind::Hard,
            }
        );
    }

    #[test]
    fn none_decision_preserves_runtime_policy() {
        assert_eq!(
            apply_pre_tool_hook_decision("t", "{}", None, false, None, None),
            PreToolHookResolution::Execute
        );
        assert_eq!(
            apply_pre_tool_hook_decision("t", "{}", Some(sample_req()), false, None, None),
            PreToolHookResolution::RequestApproval {
                request: sample_req(),
                kind: PendingApprovalKind::Standard,
            }
        );
    }

    /// The `Notification` type string a tool-approval prompt emits is the
    /// Claude Code one, so a plugin can match it by name (#3005).
    #[test]
    fn permission_prompt_is_the_claude_code_notification_type() {
        assert_eq!(
            super::NOTIFICATION_PERMISSION_PROMPT,
            "permission_prompt",
            "the emitted type string must stay the Claude Code value"
        );
        // Matchers are regexes, so an anchored pattern reaches it too.
        assert!(crate::services::hooks::matcher_matches(
            Some("permission_prompt"),
            super::NOTIFICATION_PERMISSION_PROMPT
        ));
        assert!(crate::services::hooks::matcher_matches(
            Some("^permission_.*$"),
            super::NOTIFICATION_PERMISSION_PROMPT
        ));
    }
}

#[cfg(test)]
mod pending_approval_tests {
    use super::{build_pending_approval, ToolCallRef};
    use crate::agent::events::AgentEvent;
    use crate::agent::state::PendingApprovalKind;
    use crate::agent::tool_approvals::{
        evaluate_tool_execution_policy, ToolApprovalRequest, ToolExecutionPolicyDecision,
    };
    use tokio::sync::oneshot;

    fn event_arguments(event: &AgentEvent) -> &str {
        let AgentEvent::ToolExecutionRequiresApproval { arguments, .. } = event else {
            panic!("expected a ToolExecutionRequiresApproval event");
        };
        arguments
    }

    /// #2968 (gatekeeper G1.1): the arguments stored in `pending_approvals` (the
    /// source for the Session API listing and the resume re-emit) and the
    /// arguments emitted for the approval card both come from the request's
    /// masked arguments — a literal query value, header or fragment never
    /// reaches either.
    #[tokio::test]
    async fn mcp_server_approval_stores_and_emits_masked_arguments() {
        let args = serde_json::json!({
            "name": "svc",
            "transport": {
                "type": "http",
                "url": "http://127.0.0.1:9001/mcp?token=literal-secret&key={{secret:URLKEY}}#frag",
                "headers": { "Authorization": "Bearer literal-header" },
            },
        });
        let decision = evaluate_tool_execution_policy("tool__registerServer", &args).await;
        let ToolExecutionPolicyDecision::RequireHardApproval(request) = decision else {
            panic!("expected hard approval, got {decision:?}");
        };
        let (tx, _rx) = oneshot::channel();
        let (stored, event) = build_pending_approval(
            &ToolCallRef {
                session_id: "sess-2968",
                tool_call_id: "tc-2968",
                tool_name: "tool__registerServer",
            },
            &args.to_string(),
            &request,
            PendingApprovalKind::Hard,
            tx,
            None,
        );

        assert_eq!(stored.arguments, event_arguments(&event));
        for arguments in [&stored.arguments, event_arguments(&event)] {
            assert!(!arguments.contains("literal-secret"), "{arguments}");
            assert!(!arguments.contains("literal-header"), "{arguments}");
            assert!(!arguments.contains("#frag"), "{arguments}");
            assert!(arguments.contains("token=***"), "{arguments}");
            assert!(
                arguments.contains("{{secret:URLKEY}}"),
                "templates must stay visible: {arguments}"
            );
        }
    }

    /// The opposite case: a request with no masked arguments keeps the tool
    /// call's raw arguments on both the stored record and the event, and the
    /// kind and request id reach both payloads.
    #[test]
    fn approval_without_masked_arguments_keeps_raw_arguments() {
        let request = ToolApprovalRequest {
            description: "run the thing".to_string(),
            input_preview: "preview".to_string(),
            masked_arguments: None,
        };
        let raw = r#"{"command":"ls -la"}"#;
        let (tx, _rx) = oneshot::channel();
        let (stored, event) = build_pending_approval(
            &ToolCallRef {
                session_id: "sess-raw",
                tool_call_id: "tc-raw",
                tool_name: "workspace__runShell",
            },
            raw,
            &request,
            PendingApprovalKind::Standard,
            tx,
            Some("req-9".to_string()),
        );
        assert_eq!(stored.arguments, raw);
        assert_eq!(event_arguments(&event), raw);
        assert_eq!(stored.request_id.as_deref(), Some("req-9"));
        let AgentEvent::ToolExecutionRequiresApproval {
            request_id,
            approval_kind,
            ..
        } = &event
        else {
            panic!("expected a ToolExecutionRequiresApproval event");
        };
        assert_eq!(request_id.as_deref(), Some("req-9"));
        assert_eq!(*approval_kind, PendingApprovalKind::Standard);
    }
}

#[cfg(test)]
mod hook_input_rewrite_tests {
    use super::{
        apply_hook_input_rewrite, build_pending_approval, EffectiveCall, HookInputRewrite,
        PreExecutionGuards, ToolCallRef,
    };
    use crate::agent::events::AgentEvent;
    use crate::agent::state::{AgentSession, PendingApprovalKind};
    use crate::agent::tool_approvals::{ToolApprovalRequest, ToolExecutionPolicyDecision};
    use crate::services::hooks::{HookDecision, HookDispatchResult};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::{oneshot, RwLock};

    fn empty_sessions() -> Arc<RwLock<HashMap<String, AgentSession>>> {
        Arc::new(RwLock::new(HashMap::new()))
    }

    fn allow_rewrite(updated: serde_json::Value) -> HookDispatchResult {
        HookDispatchResult {
            decision: Some(HookDecision::Allow),
            updated_input: Some(updated),
            ..HookDispatchResult::default()
        }
    }

    fn original_guards() -> PreExecutionGuards {
        PreExecutionGuards {
            policy_decision: ToolExecutionPolicyDecision::Allow,
            refusal: None,
        }
    }

    /// #3005 (first acceptance criterion, gatekeeper G1.5): the rewrite from
    /// `updatedInput` is the argument object the tool call runs with — same
    /// production step `execute_tool_calls` uses, and the same JSON string
    /// the approval and history paths carry.
    #[tokio::test]
    async fn rewritten_arguments_are_what_the_tool_receives() {
        let outcome = allow_rewrite(json!({"path": "rewritten.txt", "offset": 3}));
        let original = json!({"path": "original.txt"});

        let rewrite = apply_hook_input_rewrite(
            &empty_sessions(),
            "sess-rewrite",
            false,
            "workspace__readFile",
            &outcome,
            &original.to_string(),
            original_guards(),
        )
        .await;

        let EffectiveCall {
            args, args_json, ..
        } = match rewrite {
            HookInputRewrite::Applied(effective) => effective,
            other => panic!(
                "expected Applied, got a {} variant",
                match other {
                    HookInputRewrite::Applied(_) => "Applied",
                    HookInputRewrite::Unchanged(_) => "Unchanged",
                    HookInputRewrite::Refused(_) => "Refused",
                }
            ),
        };
        assert_eq!(args, json!({"path": "rewritten.txt", "offset": 3}));
        assert_eq!(
            args_json,
            json!({"path": "rewritten.txt", "offset": 3}).to_string()
        );
    }

    /// A rewrite that replaces the call with a destructive shell command is
    /// refused by the same policy guard that refuses the model's own
    /// `rm -rf /` — the hook cannot walk the command around it (G1.5).
    #[tokio::test]
    async fn rewrite_into_a_blocked_command_is_refused() {
        let outcome = allow_rewrite(json!({"command": "rm -rf /"}));

        let rewrite = apply_hook_input_rewrite(
            &empty_sessions(),
            "sess-rewrite-block",
            false,
            "workspace__runShell",
            &outcome,
            r#"{"command":"echo safe"}"#,
            original_guards(),
        )
        .await;

        assert!(
            matches!(rewrite, HookInputRewrite::Refused(_)),
            "rewriting runShell into a blocked command must be refused"
        );
    }

    /// A rewrite that asks for root execution is refused for a host session
    /// (G1.5), exactly like a model-originated root call.
    #[tokio::test]
    async fn rewrite_into_root_execution_is_refused() {
        let outcome = allow_rewrite(json!({"command": "id", "asRoot": true}));

        let rewrite = apply_hook_input_rewrite(
            &empty_sessions(),
            "sess-rewrite-root",
            false,
            "workspace__runShell",
            &outcome,
            r#"{"command":"id"}"#,
            original_guards(),
        )
        .await;

        assert!(
            matches!(rewrite, HookInputRewrite::Refused(_)),
            "rewriting into an asRoot call on a host session must be refused"
        );
    }

    /// A `Block` decision ignores any rewrite: the call never runs, and the
    /// original arguments and guards stand.
    #[tokio::test]
    async fn block_decision_drops_the_rewrite() {
        let outcome = HookDispatchResult {
            decision: Some(HookDecision::Block),
            reason: Some("denied".to_string()),
            updated_input: Some(json!({"path": "evil.txt"})),
            ..HookDispatchResult::default()
        };

        let rewrite = apply_hook_input_rewrite(
            &empty_sessions(),
            "sess-rewrite-blocked-decision",
            false,
            "workspace__readFile",
            &outcome,
            r#"{"path":"original.txt"}"#,
            original_guards(),
        )
        .await;

        assert!(matches!(rewrite, HookInputRewrite::Unchanged(_)));
    }

    /// The approval card shows the rewritten arguments, not the model's:
    /// the applied `args_json` is what the loop hands to the pending-approval
    /// record and event (#3005, gatekeeper G1.5).
    #[tokio::test]
    async fn approval_request_shows_the_rewritten_arguments() {
        let outcome = allow_rewrite(json!({"command": "echo rewritten"}));
        let rewrite = apply_hook_input_rewrite(
            &empty_sessions(),
            "sess-rewrite-approval",
            false,
            "workspace__runShell",
            &outcome,
            r#"{"command":"echo original"}"#,
            original_guards(),
        )
        .await;
        let HookInputRewrite::Applied(effective) = rewrite else {
            panic!("expected the rewrite to apply");
        };

        let request = ToolApprovalRequest {
            description: "run the thing".to_string(),
            input_preview: "preview".to_string(),
            masked_arguments: None,
        };
        let (tx, _rx) = oneshot::channel();
        let (stored, event) = build_pending_approval(
            &ToolCallRef {
                session_id: "sess-rewrite-approval",
                tool_call_id: "tc-rewrite",
                tool_name: "workspace__runShell",
            },
            &effective.args_json,
            &request,
            PendingApprovalKind::Standard,
            tx,
            None,
        );

        assert!(
            stored.arguments.contains("rewritten"),
            "{:?}",
            stored.arguments
        );
        assert!(
            !stored.arguments.contains("original"),
            "{:?}",
            stored.arguments
        );
        let AgentEvent::ToolExecutionRequiresApproval { arguments, .. } = &event else {
            panic!("expected a ToolExecutionRequiresApproval event");
        };
        assert!(arguments.contains("rewritten"), "{arguments}");
        assert!(!arguments.contains("original"), "{arguments}");
    }
}
