use super::approval::{ApprovalOutcome, ToolExecutionContext};
use super::results::{
    args_parse_error_result, cancelled_tool_result, error_result,
    inject_cancel_tombstones_for_remaining, session_cancel_requested, tool_result_from_mcp_result,
};
use crate::agent::llm::natural_recovery::{
    build_loop_prevention_guidance, loop_prevention_tool_result, LoopPreventionShortCircuit,
};
use crate::agent::llm::tool_args_validation::inspect_tool_call_arguments;
use crate::agent::state::{AgentSession, PendingApprovalKind};
use crate::agent::tool_approvals::ToolExecutionPolicyDecision;
use crate::agent::types::{ToolCall, ToolCallFunction};
use crate::commands::agent_commands::ToolCancellation;
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::SessionRepository;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

enum GateStep {
    Execute,
    NextCall,
    StopBatch,
}

struct ToolCallInput<'a> {
    id: &'a str,
    name: &'a str,
    args_str: &'a str,
    args: &'a serde_json::Value,
}

enum PreparedCall {
    Execute(serde_json::Value),
    NextCall,
    StopBatch,
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

        let tool_call = &tool_calls[index];
        match prepare_and_gate_tool_call(
            &context,
            tool_call,
            loop_prevention_short_circuits.get(&tool_call.id),
            &tool_calls[index + 1..],
        )
        .await
        {
            PreparedCall::Execute(args) => {
                run_and_record_tool_call(
                    &context,
                    &cancellation_token,
                    &tool_call.id,
                    &tool_call.function.name,
                    args,
                )
                .await;
                index += 1;
            }
            PreparedCall::NextCall => index += 1,
            PreparedCall::StopBatch => break,
        }
    }
}

async fn prepare_and_gate_tool_call(
    context: &ToolExecutionContext<'_>,
    tool_call: &ToolCall,
    short_circuit: Option<&LoopPreventionShortCircuit>,
    remaining_calls: &[ToolCall],
) -> PreparedCall {
    let ToolCall {
        id: tool_call_id,
        function,
        ..
    } = tool_call;
    let ToolCallFunction {
        name: tool_name,
        arguments: args_str,
    } = function;

    context.emit_tool_execution_started(tool_name);

    if let Some(short_circuit) = short_circuit {
        let guidance = build_loop_prevention_guidance(short_circuit);
        context
            .continue_after_tool(
                tool_call_id,
                loop_prevention_tool_result(&guidance),
                "loop prevention short-circuit",
            )
            .await;
        return PreparedCall::NextCall;
    }

    let Some(args) = parse_tool_call_args(context, tool_call_id, tool_name, args_str).await else {
        return PreparedCall::NextCall;
    };

    let Some((policy_decision, unsafe_enabled)) =
        check_security_and_root_guards(context, tool_call_id, tool_name, &args).await
    else {
        return PreparedCall::NextCall;
    };

    let call = ToolCallInput {
        id: tool_call_id,
        name: tool_name,
        args_str,
        args: &args,
    };
    match dispatch_pre_tool_hook_and_gate(
        context,
        &call,
        &policy_decision,
        unsafe_enabled,
        remaining_calls,
    )
    .await
    {
        GateStep::Execute => PreparedCall::Execute(args),
        GateStep::NextCall => PreparedCall::NextCall,
        GateStep::StopBatch => PreparedCall::StopBatch,
    }
}

async fn parse_tool_call_args(
    context: &ToolExecutionContext<'_>,
    tool_call_id: &str,
    tool_name: &str,
    args_str: &str,
) -> Option<serde_json::Value> {
    match inspect_tool_call_arguments(args_str) {
        Ok(map) => Some(serde_json::Value::Object(map)),
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
            None
        }
    }
}

async fn check_security_and_root_guards(
    context: &ToolExecutionContext<'_>,
    tool_call_id: &str,
    tool_name: &str,
    args: &serde_json::Value,
) -> Option<(ToolExecutionPolicyDecision, bool)> {
    let policy_decision =
        crate::agent::tool_approvals::evaluate_tool_execution_policy(tool_name, args).await;

    let unsafe_enabled = context.current_unsafe_mode().await;
    if let Some(blocked) = crate::agent::tool_approvals::blocked_execution_for_runtime(
        &policy_decision,
        unsafe_enabled,
    ) {
        context
            .continue_after_tool(
                tool_call_id,
                error_result(blocked.message.clone()),
                "policy-blocked tool execution",
            )
            .await;
        return None;
    }

    // Only the in-memory metadata is read: a session that is not loaded gets the host refusal.
    let root_refusal = {
        let active = context.active_sessions.read().await;
        crate::agent::tool_approvals::root_execution_refusal(
            tool_name,
            args,
            active
                .get(context.session_id)
                .map(|session| &session.metadata),
        )
    };
    if let Some(refusal) = root_refusal {
        context
            .continue_after_tool(
                tool_call_id,
                tool_result_from_mcp_result(refusal),
                "root execution unavailable",
            )
            .await;
        return None;
    }

    Some((policy_decision, unsafe_enabled))
}

async fn dispatch_pre_tool_hook_and_gate(
    context: &ToolExecutionContext<'_>,
    call: &ToolCallInput<'_>,
    policy_decision: &ToolExecutionPolicyDecision,
    unsafe_enabled: bool,
    remaining_calls: &[ToolCall],
) -> GateStep {
    // PreToolUse hook (#2925): runs after hard security/root guards and before
    // approval/execution. Applies matcher on `tool_name` and `permissionDecision`
    // (allow / deny / ask).
    let mut pre_tool_params = crate::services::hooks::HookEnvelopeParams::new(
        context.session_id,
        std::path::PathBuf::from("."),
        crate::services::hooks::HookEvent::PreToolUse,
    );
    pre_tool_params.tool_name = Some(call.name.to_string());
    pre_tool_params.tool_input = Some(call.args.clone());
    let pre_tool_outcome = crate::services::hooks::dispatch_hook_event(
        crate::services::hooks::HookEvent::PreToolUse,
        call.name,
        &pre_tool_params,
        context.session_id,
    )
    .await;
    if let Some(ctx) = pre_tool_outcome.joined_additional_context() {
        crate::services::hooks::push_turn_hook_context(context.session_id, &ctx);
    }

    let yolo_enabled = context.current_yolo_mode().await;
    let is_hard_approval = matches!(
        policy_decision,
        ToolExecutionPolicyDecision::RequireHardApproval(_)
    );
    let base_approval_request = crate::agent::tool_approvals::approval_request_for_runtime(
        policy_decision,
        yolo_enabled,
        unsafe_enabled,
    )
    .cloned();

    match apply_pre_tool_hook_decision(
        call.name,
        call.args_str,
        base_approval_request,
        is_hard_approval,
        pre_tool_outcome.decision,
        pre_tool_outcome.reason,
    ) {
        PreToolHookResolution::Block { reason } => {
            context
                .continue_after_tool(call.id, error_result(reason), "hook-blocked tool execution")
                .await;
            GateStep::NextCall
        }
        PreToolHookResolution::Execute => GateStep::Execute,
        PreToolHookResolution::RequestApproval {
            request: approval_request,
            kind: approval_kind,
        } => {
            handle_approval_outcome(
                context,
                call,
                &approval_request,
                approval_kind,
                remaining_calls,
            )
            .await
        }
    }
}

async fn handle_approval_outcome(
    context: &ToolExecutionContext<'_>,
    call: &ToolCallInput<'_>,
    approval_request: &crate::agent::tool_approvals::ToolApprovalRequest,
    approval_kind: PendingApprovalKind,
    remaining_calls: &[ToolCall],
) -> GateStep {
    match context
        .request_approval(
            call.id,
            call.name,
            call.args_str,
            approval_request,
            approval_kind,
        )
        .await
    {
        ApprovalOutcome::Approved => GateStep::Execute,
        ApprovalOutcome::Rejected => {
            context
                .continue_after_tool(
                    call.id,
                    error_result("User rejected the tool execution."),
                    "tool rejection",
                )
                .await;
            // User rejected this tool — cancel remaining calls in the batch too.
            inject_cancel_tombstones_for_remaining(context, remaining_calls).await;
            GateStep::StopBatch
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
                return GateStep::StopBatch;
            }

            // Cancel/terminate dropped the approval waiter while the session is still active and busy.
            // Close this call and let the cancel check at the top tombstone any remainder.
            context
                .continue_after_tool(call.id, cancelled_tool_result(), "approval cancelled")
                .await;
            GateStep::NextCall
        }
    }
}

async fn run_and_record_tool_call(
    context: &ToolExecutionContext<'_>,
    cancellation_token: &CancellationToken,
    tool_call_id: &str,
    tool_name: &str,
    args: serde_json::Value,
) {
    // `args` is moved into `execute_tool` below; keep a copy for the
    // PostToolUse hook envelope.
    let post_tool_input = args.clone();
    let execution_started = std::time::Instant::now();
    let (mut result, process_was_cancelled) = context
        .proxy_manager
        .run_with_process_cancel_scope(
            context.session_id,
            crate::agent::state::with_tool_call_id(
                tool_call_id.to_string(),
                context.execute_tool(tool_name, args),
            ),
        )
        .await;
    result.execution_duration_ms = Some(execution_started.elapsed().as_millis() as u64);
    if result.is_error
        && (process_was_cancelled
            || session_cancel_requested(
                context.active_sessions,
                context.session_id,
                cancellation_token,
            )
            .await)
    {
        result.cancellation = Some(ToolCancellation::user());
    }
    let tool_ended_with_error = result.is_error;
    context
        .continue_after_tool(tool_call_id, result, "tool execution")
        .await;

    if !tool_ended_with_error {
        dispatch_post_tool_use_hook(context.session_id, tool_name, post_tool_input).await;
    }
}

async fn dispatch_post_tool_use_hook(
    session_id: &str,
    tool_name: &str,
    post_tool_input: serde_json::Value,
) {
    // PostToolUse hook (#2925): runs after a successful tool call. Adds
    // `additionalContext` and `decision: "block"` feedback to the next LLM request.
    let mut post_tool_params = crate::services::hooks::HookEnvelopeParams::new(
        session_id,
        std::path::PathBuf::from("."),
        crate::services::hooks::HookEvent::PostToolUse,
    );
    post_tool_params.tool_name = Some(tool_name.to_string());
    post_tool_params.tool_input = Some(post_tool_input);
    post_tool_params.tool_response = Some(serde_json::json!({ "is_error": false }));
    let post_tool_outcome = crate::services::hooks::dispatch_hook_event(
        crate::services::hooks::HookEvent::PostToolUse,
        tool_name,
        &post_tool_params,
        session_id,
    )
    .await;
    if let Some(ctx) = post_tool_outcome.joined_additional_context() {
        crate::services::hooks::push_turn_hook_context(session_id, &ctx);
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
            session_id,
            &format!("[PostToolUse hook block]: {reason}"),
        );
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
