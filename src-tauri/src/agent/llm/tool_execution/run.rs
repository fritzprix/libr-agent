use super::approval::{ApprovalOutcome, ToolExecutionContext};
use super::results::{
    args_parse_error_result, cancelled_tool_result, error_result,
    inject_cancel_tombstones_for_remaining, session_cancel_requested, tool_result_from_mcp_result,
};
use crate::agent::events::AgentEventDispatcher;
use crate::agent::llm::natural_recovery::{
    build_loop_prevention_guidance, loop_prevention_tool_result, LoopPreventionShortCircuit,
};
use crate::agent::llm::tool_args_validation::inspect_tool_call_arguments;
use crate::agent::state::{AgentSession, PendingApprovalKind};
use crate::agent::tauri_events::TauriEventDispatcher;
use crate::agent::tool_approvals::ToolExecutionPolicyDecision;
use crate::agent::types::{ToolCall, ToolCallFunction};
use crate::commands::agent_commands::{ToolCancellation, ToolExecutionResult};
use crate::mcp::MCPServiceProxyManager;
use crate::repositories::SessionRepository;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
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
}

enum PreparedCall {
    Execute(serde_json::Value),
    NextCall,
    StopBatch,
    HookStopped(String),
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
                if let Some(stop_reason) = run_and_record_tool_call(
                    &context,
                    &cancellation_token,
                    &tool_call.id,
                    &tool_call.function.name,
                    args,
                )
                .await
                {
                    stop_turn_from_hook(
                        context.session_repo,
                        context.active_sessions,
                        context.proxy_manager,
                        context.app_handle,
                        context.session_id,
                        &tool_calls[index + 1..],
                        &stop_reason,
                    )
                    .await;
                    break;
                }
                index += 1;
            }
            PreparedCall::NextCall => index += 1,
            PreparedCall::StopBatch => break,
            PreparedCall::HookStopped(stop_reason) => {
                cancellation_token.cancel();
                context
                    .continue_after_tool(
                        &tool_call.id,
                        error_result(stop_reason.clone()),
                        "hook-stopped tool execution",
                    )
                    .await;
                stop_turn_from_hook(
                    context.session_repo,
                    context.active_sessions,
                    context.proxy_manager,
                    context.app_handle,
                    context.session_id,
                    &tool_calls[index + 1..],
                    &stop_reason,
                )
                .await;
                break;
            }
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

    let unsafe_enabled = context.current_unsafe_mode().await;
    let mut guards = pre_execution_guards(
        context.active_sessions,
        context.session_id,
        unsafe_enabled,
        tool_name,
        &args,
    )
    .await;
    if let Some((refusal, reason)) = guards.refusal.take() {
        context
            .continue_after_tool(tool_call_id, refusal, reason)
            .await;
        return PreparedCall::NextCall;
    }

    dispatch_pre_tool_hook_and_gate(
        context,
        tool_call,
        args,
        guards,
        unsafe_enabled,
        remaining_calls,
    )
    .await
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

/// Pre-execution guards for one argument object (#3005).
pub(super) struct PreExecutionGuards {
    pub(super) policy_decision: ToolExecutionPolicyDecision,
    /// The result to return instead of executing, plus its error context.
    pub(super) refusal: Option<(ToolExecutionResult, &'static str)>,
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
pub(super) struct EffectiveCall {
    pub(super) args: serde_json::Value,
    pub(super) args_json: String,
    pub(super) guards: PreExecutionGuards,
}

/// Outcome of the `updatedInput` step of the tool loop. Every non-`Applied`
/// outcome carries the original guards back so the loop keeps them.
pub(super) enum HookInputRewrite {
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
pub(super) async fn apply_hook_input_rewrite(
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

struct PreToolHookStage {
    args: serde_json::Value,
    args_json: String,
    guards: PreExecutionGuards,
    outcome: crate::services::hooks::HookDispatchResult,
}

async fn dispatch_pre_tool_hook_and_rewrite(
    context: &ToolExecutionContext<'_>,
    tool_call: &ToolCall,
    args: serde_json::Value,
    guards: PreExecutionGuards,
    unsafe_enabled: bool,
) -> Option<PreToolHookStage> {
    let tool_call_id = tool_call.id.as_str();
    let tool_name = tool_call.function.name.as_str();
    let args_str = tool_call.function.arguments.as_str();
    // PreToolUse hook (#2925): runs after hard security/root guards and before
    // approval/execution. Applies matcher on `tool_name` and `permissionDecision`
    // (allow / deny / ask).
    let mut pre_tool_params = crate::services::hooks::HookEnvelopeParams::new(
        context.session_id,
        std::path::PathBuf::from("."),
        crate::services::hooks::HookEvent::PreToolUse,
    );
    pre_tool_params.tool_name = Some(tool_name.to_string());
    pre_tool_params.tool_input = Some(args.clone());
    let pre_tool_outcome = crate::services::hooks::dispatch_hook_event(
        crate::services::hooks::HookEvent::PreToolUse,
        tool_name,
        &pre_tool_params,
        context.session_id,
    )
    .await;
    if let Some(ctx) = pre_tool_outcome.joined_additional_context() {
        crate::services::hooks::push_turn_hook_context(context.session_id, &ctx);
    }

    // `updatedInput` (#3005): apply the hook's rewrite through the
    // production step `apply_hook_input_rewrite` — object-only, Block
    // drops it, and the rewritten arguments re-run every guard.
    let rewrite = apply_hook_input_rewrite(
        context.active_sessions,
        context.session_id,
        unsafe_enabled,
        tool_name,
        &pre_tool_outcome,
        args_str,
        guards,
    )
    .await;
    let (args, guards, args_json) = match rewrite {
        HookInputRewrite::Unchanged(guards) => (args, guards, args_str.to_string()),
        HookInputRewrite::Refused((refusal, reason)) => {
            context
                .continue_after_tool(tool_call_id, refusal, reason)
                .await;
            return None;
        }
        HookInputRewrite::Applied(effective) => {
            (effective.args, effective.guards, effective.args_json)
        }
    };
    Some(PreToolHookStage {
        args,
        args_json,
        guards,
        outcome: pre_tool_outcome,
    })
}

async fn dispatch_pre_tool_hook_and_gate(
    context: &ToolExecutionContext<'_>,
    tool_call: &ToolCall,
    args: serde_json::Value,
    guards: PreExecutionGuards,
    unsafe_enabled: bool,
    remaining_calls: &[ToolCall],
) -> PreparedCall {
    let Some(stage) =
        dispatch_pre_tool_hook_and_rewrite(context, tool_call, args, guards, unsafe_enabled).await
    else {
        return PreparedCall::NextCall;
    };

    if stage.outcome.stops_processing {
        let stop_reason = stage
            .outcome
            .stop_message("PreToolUse hook stopped processing.");
        return PreparedCall::HookStopped(stop_reason);
    }

    let policy_decision = &stage.guards.policy_decision;
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

    let call = ToolCallInput {
        id: tool_call.id.as_str(),
        name: tool_call.function.name.as_str(),
        args_str: stage.args_json.as_str(),
    };
    match apply_pre_tool_hook_decision(
        call.name,
        call.args_str,
        base_approval_request,
        is_hard_approval,
        stage.outcome.decision,
        stage.outcome.reason,
    ) {
        PreToolHookResolution::Block { reason } => {
            context
                .continue_after_tool(call.id, error_result(reason), "hook-blocked tool execution")
                .await;
            PreparedCall::NextCall
        }
        PreToolHookResolution::Execute => PreparedCall::Execute(stage.args),
        PreToolHookResolution::RequestApproval {
            request: approval_request,
            kind: approval_kind,
        } => {
            match handle_approval_outcome(
                context,
                &call,
                &approval_request,
                approval_kind,
                remaining_calls,
            )
            .await
            {
                GateStep::Execute => PreparedCall::Execute(stage.args),
                GateStep::NextCall => PreparedCall::NextCall,
                GateStep::StopBatch => PreparedCall::StopBatch,
            }
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
) -> Option<String> {
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
    // Captured before the result is moved: the `PostToolUse` envelope carries
    // the real tool result (#3004).
    let post_tool_envelope = (!tool_ended_with_error).then(|| post_tool_response(&result));

    // PostToolUse hook (#2925, #3004 G1.2): evaluate BEFORE `continue_after_tool`
    // so `additionalContext`, `block` feedback, and `continue: false` take effect
    // before the next completion is requested.
    let post_tool_stop_reason = if !tool_ended_with_error {
        dispatch_post_tool_use_hook(
            context.session_id,
            tool_name,
            post_tool_input,
            post_tool_envelope,
            cancellation_token,
        )
        .await
    } else {
        None
    };

    context
        .continue_after_tool(tool_call_id, result, "tool execution")
        .await;

    post_tool_stop_reason
}

async fn dispatch_post_tool_use_hook(
    session_id: &str,
    tool_name: &str,
    post_tool_input: serde_json::Value,
    post_tool_envelope: Option<serde_json::Value>,
    cancellation_token: &CancellationToken,
) -> Option<String> {
    // PostToolUse hook (#2925): runs after a successful tool call. Adds
    // `additionalContext` and `decision: "block"` feedback to the next LLM request.
    let mut post_tool_params = crate::services::hooks::HookEnvelopeParams::new(
        session_id,
        std::path::PathBuf::from("."),
        crate::services::hooks::HookEvent::PostToolUse,
    );
    post_tool_params.tool_name = Some(tool_name.to_string());
    post_tool_params.tool_input = Some(post_tool_input);
    post_tool_params.tool_response = post_tool_envelope;
    let post_tool_outcome = crate::services::hooks::dispatch_hook_event(
        crate::services::hooks::HookEvent::PostToolUse,
        tool_name,
        &post_tool_params,
        session_id,
    )
    .await;
    if post_tool_outcome.stops_processing {
        let stop_reason = post_tool_outcome.stop_message("PostToolUse hook stopped processing.");
        cancellation_token.cancel();
        return Some(stop_reason);
    }
    if let Some(ctx) = post_tool_outcome.joined_additional_context() {
        crate::services::hooks::push_turn_hook_context(session_id, &ctx);
    }
    if let Some(reason) = post_tool_feedback(&post_tool_outcome, tool_name) {
        crate::services::hooks::push_turn_hook_context(
            session_id,
            &format!("[PostToolUse hook block]: {reason}"),
        );
    }
    None
}

/// Tool result recorded for remaining batch calls skipped after a hook returned
/// `continue: false` (#3004 G2.1).
fn hook_stopped_remaining_tool_result(stop_reason: &str) -> ToolExecutionResult {
    error_result(format!(
        "Skipped because hook stopped processing: {stop_reason}"
    ))
}

/// Finish a turn stopped by a `PreToolUse` / `PostToolUse` hook (`continue: false`,
/// #3004 G2.1/G3.2): label remaining calls as stopped by hook, then end the turn
/// through the **shared settlement code** so queued prompts keep draining and the
/// `Stop` hook decision is made there (a hook already stopped this turn, so it does
/// not run) — and so `WorkflowCompleted` carries `stopReason` for the UI to show on
/// a non-error end of turn (#3004 G3.1).
///
/// `pub` so the integration test can drive the production stop path (#3004 G2.2).
pub async fn stop_turn_from_hook(
    session_repo: &Arc<dyn SessionRepository>,
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    proxy_manager: &Arc<MCPServiceProxyManager>,
    app_handle: &AppHandle,
    session_id: &str,
    remaining: &[ToolCall],
    stop_reason: &str,
) {
    // Capture user cancel BEFORE submitting remaining results (#3004 G2.2):
    // completing the batch inside `continue_after_tool` consumes `cancel_pending`
    // and transitions status to `Paused`, which would otherwise hide the cancel
    // from the check below.
    let user_cancelled_before = {
        let sessions = active_sessions.read().await;
        sessions.get(session_id).is_some_and(|session| {
            session.cancel_pending.load(Ordering::SeqCst)
                || session.metadata.status == crate::repositories::SessionStatus::Paused
        })
    };

    let context = ToolExecutionContext {
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        session_id,
    };
    for tool_call in remaining {
        context
            .continue_after_tool(
                &tool_call.id,
                hook_stopped_remaining_tool_result(stop_reason),
                "hook-stopped remaining tool call",
            )
            .await;
    }

    let user_cancelled = {
        let mut sessions = active_sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            if user_cancelled_before
                || session.cancel_pending.load(Ordering::SeqCst)
                || session.metadata.status == crate::repositories::SessionStatus::Paused
            {
                true
            } else {
                // The caller cancelled this turn's `cancellation_token`, but
                // settlement below may drain a queued prompt into a brand-new
                // turn. Reset exactly like `start_workflow` does so the
                // continuation starts clean (#3004 G4.1).
                crate::agent::workflow::reset_session_execution_state(session).await;
                false
            }
        } else {
            user_cancelled_before
        }
    };
    let dispatcher = TauriEventDispatcher::from_handle(app_handle);
    if user_cancelled {
        crate::services::hooks::reset_stop_hook_continuations(session_id);
        if let Err(error) = crate::agent::lifecycle::update_session_status(
            session_repo,
            active_sessions,
            app_handle,
            session_id,
            crate::repositories::SessionStatus::Paused,
        )
        .await
        {
            log::error!(
                "Failed to settle user-cancelled session {session_id} to Paused after hook stop: {error}"
            );
        }
        if let Err(error) =
            dispatcher.emit_agent_event(crate::agent::events::AgentEvent::WorkflowCompleted {
                session_id: session_id.to_string(),
                reason: crate::agent::events::WorkflowCompletionReason::Cancelled,
                stop_reason: None,
            })
        {
            log::error!(
                "Failed to emit Cancelled WorkflowCompleted for session {session_id}: {error}"
            );
        }
        return;
    }
    if let Err(error) = crate::agent::workflow::settle_session_and_go_idle_with_dispatcher(
        session_repo,
        active_sessions,
        proxy_manager,
        app_handle,
        &dispatcher,
        session_id,
        None,
        crate::agent::events::WorkflowCompletionReason::Natural,
        Some(stop_reason),
    )
    .await
    {
        log::error!("Failed to settle session {session_id} to Idle after hook stop: {error}");
    }
}

/// Feedback a `PostToolUse` hook wants the agent to see before the next request
/// (#3004): a `block` decision surfaces its `reason`. (`continue: false` is
/// handled before `continue_after_tool` so the turn ends immediately.)
fn post_tool_feedback(
    outcome: &crate::services::hooks::HookDispatchResult,
    tool_name: &str,
) -> Option<String> {
    if outcome.decision == Some(crate::services::hooks::HookDecision::Block) {
        return Some(
            outcome
                .reason
                .clone()
                .filter(|r| !r.trim().is_empty())
                .unwrap_or_else(|| {
                    format!("PostToolUse hook blocked continuation after '{tool_name}'.")
                }),
        );
    }
    None
}

/// Envelope `tool_response` for a `PostToolUse` hook (#3004).
///
/// The hook sees the real tool result, not a `{ "is_error": false }` stub, in the
/// same shape the MCP layer produced.
fn post_tool_response(
    result: &crate::commands::agent_commands::ToolExecutionResult,
) -> serde_json::Value {
    serde_json::json!({
        "success": result.success,
        "is_error": result.is_error,
        "content": result.content,
        "structured_content": result.structured_content,
        "error": result.error,
    })
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
mod hook_stop_and_tool_response_tests {
    use super::{hook_stopped_remaining_tool_result, post_tool_feedback, post_tool_response};
    use crate::commands::agent_commands::ToolExecutionResult;
    use crate::services::hooks::{HookDecision, HookDispatchResult};

    fn empty_result() -> HookDispatchResult {
        HookDispatchResult {
            additional_context: vec![],
            decision: None,
            reason: None,
            updated_input: None,
            stop_reason: None,
            stops_processing: false,
        }
    }

    #[test]
    fn hook_stopped_remaining_tool_result_labels_hook_stop_not_user_cancel() {
        let res = hook_stopped_remaining_tool_result("budget exhausted");
        assert!(res.is_error);
        assert!(res.cancellation.is_none());
        assert_eq!(
            res.error.as_deref(),
            Some("Skipped because hook stopped processing: budget exhausted")
        );
    }

    #[test]
    fn post_tool_feedback_reports_block_only() {
        assert_eq!(
            post_tool_feedback(&empty_result(), "workspace__readFile"),
            None
        );

        let blocked = HookDispatchResult {
            decision: Some(HookDecision::Block),
            reason: Some("read the docs first".to_string()),
            ..empty_result()
        };
        assert_eq!(
            post_tool_feedback(&blocked, "workspace__readFile").as_deref(),
            Some("read the docs first")
        );
    }

    #[test]
    fn tool_response_carries_the_real_result() {
        let tool_result = ToolExecutionResult {
            success: true,
            content: "line 42: needle".to_string(),
            mcp_content: None,
            structured_content: Some(serde_json::json!({ "matches": 1 })),
            error: None,
            is_error: false,
            cancellation: None,
            execution_duration_ms: Some(7),
        };
        let response = post_tool_response(&tool_result);
        assert_eq!(response["is_error"], serde_json::json!(false));
        assert_eq!(response["content"], serde_json::json!("line 42: needle"));
        assert_eq!(
            response["structured_content"]["matches"],
            serde_json::json!(1)
        );
        assert_ne!(response, serde_json::json!({ "is_error": false }));
    }
}
