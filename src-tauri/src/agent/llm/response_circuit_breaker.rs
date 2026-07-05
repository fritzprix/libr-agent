use crate::agent::llm::circuit_breaker;
use crate::agent::llm::completion::load_context_management_settings;
use crate::agent::llm::completion::request::apply_compact_summary_projection;
use crate::agent::llm::token_utils::{
    calculate_conservative_preflight_prompt_tokens, calculate_context_safety_margin,
    calculate_effective_input_budget, calculate_prompt_anchored_total_tokens,
    derive_measured_output_tokens_reserve, estimate_text_tokens, try_derive_bpe_calibration_ratio,
};
use crate::agent::state::{AgentSession, CompactionRecoveryPhase};
use crate::agent::tools::{
    create_tool_result_message, tool_result_inline_limit_bytes,
    tool_result_preview_content_limit_bytes,
};
use crate::agent::types::{ToolCall, ToolCallFunction};
use crate::models::chat::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

const TOOL_LOOP_FENCE_MIN_KEEP_COUNT: usize = 1;
const TOOL_LOOP_FENCE_RESULT_FILLER: &str = "x";

pub(crate) async fn preprocess_assistant_tool_calls(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    assistant_message: &mut Message,
) {
    let mut forced_circuit_break_message = None;

    if let Some(tool_calls) = &mut assistant_message.tool_calls {
        let loop_threshold = circuit_breaker::load_loop_prevention_threshold().await;

        let (session_metadata, break_index, break_action) = {
            let sessions = active_sessions.read().await;
            match sessions.get(session_id) {
                None => (None, None, None),
                Some(session) => {
                    let metadata = session.metadata.clone();
                    let messages = session.messages.read().await;
                    let call_signature_by_id = circuit_breaker::build_tool_call_indices(&messages);

                    let mut break_index = None;
                    let mut break_action = None;
                    for (index, tool_call) in tool_calls.iter().enumerate() {
                        if let Some(action) = circuit_breaker::evaluate_circuit_breaker_action(
                            &messages,
                            tool_call,
                            &call_signature_by_id,
                            loop_threshold,
                        ) {
                            break_index = Some(index);
                            break_action = Some(action);
                            break;
                        }
                    }

                    (Some(metadata), break_index, break_action)
                }
            }
        };

        if let Some(index) = break_index {
            if let Some(action) = break_action {
                match action {
                    circuit_breaker::CircuitBreakerAction::HardBreak {
                        count,
                        tool_name,
                        args,
                    } => {
                        log::warn!(
                            "Circuit breaker triggered for session {} tool {} (count {})",
                            session_id,
                            tool_name,
                            count
                        );

                        let ui_alias_enabled = match session_metadata.as_ref() {
                            Some(metadata) => {
                                match crate::agent::resolve_agent_config(metadata).await {
                                    Ok(config) => {
                                        circuit_breaker::is_builtin_alias_enabled(&config, "ui")
                                    }
                                    Err(error) => {
                                        log::warn!(
                                            "Failed to resolve agent config for circuit breaker in session {}: {}",
                                            session_id,
                                            error
                                        );
                                        true
                                    }
                                }
                            }
                            None => true,
                        };

                        if ui_alias_enabled {
                            let circuit_break_call = ToolCall {
                                id: uuid::Uuid::new_v4().to_string(),
                                function: ToolCallFunction {
                                    name: "ui__circuitBreak".to_string(),
                                    arguments: serde_json::json!({
                                        "toolName": tool_name,
                                        "repetitionCount": count,
                                        "args": args
                                    })
                                    .to_string(),
                                },
                                r#type: "function".to_string(),
                            };

                            tool_calls[index] = circuit_break_call;
                            tool_calls.truncate(index + 1);
                        } else {
                            log::warn!(
                                "UI alias disabled for session {}. Using text-only circuit break fallback.",
                                session_id
                            );

                            forced_circuit_break_message =
                                Some(crate::mcp::types::MCPContent::Text {
                                    text: format!(
                                        "⚠️ Circuit breaker triggered: detected runaway loop for tool '{}' (count {}).\n\nThe 'ui' builtin server is disabled for this session, so interactive circuit-break UI was skipped. Workflow was force-stopped to prevent further runaway calls.",
                                        tool_name, count
                                    ),
                                    is_error: None,
                                });
                        }
                    }
                    circuit_breaker::CircuitBreakerAction::NaturalRecoveryError {
                        count,
                        tool_name,
                        ..
                    } => {
                        log::warn!(
                            "Natural recovery (Error track) triggered for session {} tool {} (count {})",
                            session_id, tool_name, count
                        );
                        let entropy = uuid::Uuid::new_v4().to_string();
                        let nanos = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .subsec_nanos() as usize;

                        let error_templates = [
                            (
                                "I am repeatedly calling '{TOOL_NAME}' with identical parameters. I need to determine if I am waiting for an external change, if I need to pivot, or if I am completely blocked.",
                                "I must choose one of three paths to break this loop: (1) If I am waiting for a background process, compilation, or file change, I should execute a sleep/delay command (e.g., sleep 5 or timeout 5). (2) If I am repeating this tool unnecessarily, I must review my progress and pivot to a different tool other than '{TOOL_NAME}'. (3) If I cannot proceed further, I should stop and report my current progress and the specific blocker to the user."
                            ),
                            (
                                "Executing '{TOOL_NAME}' multiple times with the same arguments has not moved the task forward. I must evaluate my situation and choose the correct recovery strategy.",
                                "I need to select the most appropriate action from these three scenarios: (1) If I am polling for an asynchronous state change, I should run a delay/sleep command. (2) If I am stuck in a logical loop, I must analyze what I have done, re-read the user request, and switch to a different tool. (3) If I have exhausted all options and cannot proceed, I must immediately report the progress and the problems to the user."
                            ),
                            (
                                "I have called '{TOOL_NAME}' consecutively without changing the state. I must stop this pattern and decide how to resolve this bottleneck.",
                                "I will choose one of the following three steps: (1) If I am waiting on a slow resource, server, or process, I will use a sleep command to pause. (2) If this repeated call is redundant, I will refocus on the goal and pivot to a different tool. (3) If I am completely blocked and cannot make further progress, I will report the current status and the obstacles directly to the user."
                            ),
                            (
                                "My repeated invocation of '{TOOL_NAME}' indicates a loop. I need to diagnose the cause of this repetition and select the correct path forward.",
                                "I must act based on one of these three scenarios: (1) If I need to wait for a state transition, I will call a sleep/timeout command to introduce a delay. (2) If I am stuck, I will review the conversation history and use a tool other than '{TOOL_NAME}' to make progress. (3) If no further progress is possible, I will stop and write a report to the user explaining my progress and the blockers."
                            )
                        ];

                        let (thought_tmpl, next_action_tmpl) =
                            error_templates[nanos % error_templates.len()];
                        let recovery_thought = format!(
                            "{} [Entropy ID: {}]",
                            thought_tmpl.replace("{TOOL_NAME}", &tool_name),
                            entropy
                        );
                        let next_action = next_action_tmpl.replace("{TOOL_NAME}", &tool_name);

                        let think_call = ToolCall {
                            id: uuid::Uuid::new_v4().to_string(),
                            function: ToolCallFunction {
                                name: "scratchpad__think".to_string(),
                                arguments: serde_json::json!({
                                    "thought": recovery_thought,
                                    "nextAction": next_action
                                })
                                .to_string(),
                            },
                            r#type: "function".to_string(),
                        };
                        tool_calls[index] = think_call;
                        tool_calls.truncate(index + 1);
                    }
                    circuit_breaker::CircuitBreakerAction::NaturalRecoverySuccess {
                        count,
                        tool_name,
                        ..
                    } => {
                        log::warn!(
                            "Natural recovery (Success track) triggered for session {} tool {} (count {})",
                            session_id, tool_name, count
                        );
                        let entropy = uuid::Uuid::new_v4().to_string();
                        let nanos = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .subsec_nanos() as usize;

                        let success_templates = [
                            (
                                "I am repeatedly calling '{TOOL_NAME}' with identical parameters. I need to determine if I am waiting for an external change, if I need to pivot, or if I am completely blocked.",
                                "I must choose one of three paths to break this loop: (1) If I am waiting for a background process, compilation, or file change, I should execute a sleep/delay command (e.g., sleep 5 or timeout 5). (2) If I am repeating this tool unnecessarily, I must review my progress and pivot to a different tool other than '{TOOL_NAME}'. (3) If I cannot proceed further, I should stop and report my current progress and the specific blocker to the user."
                            ),
                            (
                                "Executing '{TOOL_NAME}' multiple times with the same arguments has not moved the task forward. I must evaluate my situation and choose the correct recovery strategy.",
                                "I need to select the most appropriate action from these three scenarios: (1) If I am polling for an asynchronous state change, I should run a delay/sleep command. (2) If I am stuck in a logical loop, I must analyze what I have done, re-read the user request, and switch to a different tool. (3) If I have exhausted all options and cannot proceed, I must immediately report the progress and the problems to the user."
                            ),
                            (
                                "I have called '{TOOL_NAME}' consecutively without changing the state. I must stop this pattern and decide how to resolve this bottleneck.",
                                "I will choose one of the following three steps: (1) If I am waiting on a slow resource, server, or process, I will use a sleep command to pause. (2) If this repeated call is redundant, I will refocus on the goal and pivot to a different tool. (3) If I am completely blocked and cannot make further progress, I will report the current status and the obstacles directly to the user."
                            ),
                            (
                                "My repeated invocation of '{TOOL_NAME}' indicates a loop. I need to diagnose the cause of this repetition and select the correct path forward.",
                                "I must act based on one of these three scenarios: (1) If I need to wait for a state transition, I will call a sleep/timeout command to introduce a delay. (2) If I am stuck, I will review the conversation history and use a tool other than '{TOOL_NAME}' to make progress. (3) If no further progress is possible, I will stop and write a report to the user explaining my progress and the blockers."
                            )
                        ];

                        let (thought_tmpl, next_action_tmpl) =
                            success_templates[nanos % success_templates.len()];
                        let recovery_thought = format!(
                            "{} [Entropy ID: {}]",
                            thought_tmpl.replace("{TOOL_NAME}", &tool_name),
                            entropy
                        );
                        let next_action = next_action_tmpl.replace("{TOOL_NAME}", &tool_name);

                        let think_call = ToolCall {
                            id: uuid::Uuid::new_v4().to_string(),
                            function: ToolCallFunction {
                                name: "scratchpad__think".to_string(),
                                arguments: serde_json::json!({
                                    "thought": recovery_thought,
                                    "nextAction": next_action
                                })
                                .to_string(),
                            },
                            r#type: "function".to_string(),
                        };
                        tool_calls[index] = think_call;
                        tool_calls.truncate(index + 1);
                    }
                }
            }
        }
    }

    if let Some(circuit_break_message) = forced_circuit_break_message {
        assistant_message.tool_calls = None;
        assistant_message.content = vec![circuit_break_message];
        return;
    }

    apply_tool_loop_token_fence(active_sessions, session_id, assistant_message).await;
}

pub async fn preprocess_assistant_tool_calls_for_testing(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    assistant_message: &mut Message,
) {
    preprocess_assistant_tool_calls(active_sessions, session_id, assistant_message).await;
}

async fn apply_tool_loop_token_fence(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    assistant_message: &mut Message,
) {
    let Some(original_tool_calls) = assistant_message.tool_calls.clone() else {
        return;
    };

    if original_tool_calls.len() <= TOOL_LOOP_FENCE_MIN_KEEP_COUNT {
        return;
    }

    // Tool-loop token fence is a last-resort fallback after compaction hard failure
    // (DegradedTools recovery phase). Normal compact-mode sessions rely on compaction
    // and preflight context selection instead of redacting parallel tool batches here.
    let (messages_lock, last_request_lock, compact_context_lock, session_metadata) = {
        let sessions = active_sessions.read().await;
        let Some(session) = sessions.get(session_id) else {
            return;
        };
        if session.compaction.recovery_phase().await != CompactionRecoveryPhase::DegradedTools {
            return;
        }
        (
            Arc::clone(&session.messages),
            Arc::clone(&session.last_completion_request),
            Arc::clone(&session.compact_context),
            session.metadata.clone(),
        )
    };

    let context_settings = load_context_management_settings().await;

    let history_messages = messages_lock.read().await.clone();
    let compact_record = compact_context_lock.read().await.clone();
    let projection_history =
        apply_compact_summary_projection(session_id, &history_messages, compact_record.as_ref());
    let last_completion_request = last_request_lock.read().await.clone();
    let configured_max_output_tokens = crate::agent::resolve_agent_config(&session_metadata)
        .await
        .ok()
        .and_then(|config| config.max_tokens);
    let (system_prompt_tokens, tools_tokens) =
        estimate_parent_request_prefix_tokens(last_completion_request.as_ref());
    let fallback_calibration_ratio =
        try_derive_bpe_calibration_ratio(&projection_history, system_prompt_tokens, tools_tokens);

    let output_reserve_tokens =
        derive_measured_output_tokens_reserve(&projection_history, configured_max_output_tokens);
    let safe_input_token_limit = context_settings
        .max_input_context()
        .min(context_settings.model_max_limit());
    let effective_input_budget =
        calculate_effective_input_budget(safe_input_token_limit, output_reserve_tokens);
    let guarded_total_budget_limit = effective_input_budget
        .saturating_sub(calculate_context_safety_margin(effective_input_budget))
        + output_reserve_tokens;

    let inline_limit_bytes = tool_result_inline_limit_bytes().await;
    let preview_limit_bytes = tool_result_preview_content_limit_bytes(inline_limit_bytes);
    let synthetic_tool_result = TOOL_LOOP_FENCE_RESULT_FILLER.repeat(preview_limit_bytes.max(1));

    let mut kept_count = original_tool_calls.len();
    let mut kept_projection_total = 0usize;
    let mut kept_projection_prompt = 0usize;

    for prefix_len in 1..=original_tool_calls.len() {
        let projected_messages = build_projected_messages_for_prefix(
            &projection_history,
            assistant_message,
            &original_tool_calls,
            prefix_len,
            &synthetic_tool_result,
        );
        let projected_prompt_tokens = calculate_conservative_preflight_prompt_tokens(
            &projected_messages,
            system_prompt_tokens,
            tools_tokens,
            fallback_calibration_ratio,
        );
        let projected_total_budget_tokens = projected_prompt_tokens + output_reserve_tokens;

        if projected_total_budget_tokens <= guarded_total_budget_limit {
            kept_count = prefix_len;
            kept_projection_prompt = projected_prompt_tokens;
            kept_projection_total = projected_total_budget_tokens;
            continue;
        }

        if prefix_len == TOOL_LOOP_FENCE_MIN_KEEP_COUNT {
            kept_count = TOOL_LOOP_FENCE_MIN_KEEP_COUNT;
            kept_projection_prompt = projected_prompt_tokens;
            kept_projection_total = projected_total_budget_tokens;
        }
        break;
    }

    if kept_count >= original_tool_calls.len() {
        return;
    }

    let dropped_count = original_tool_calls.len() - kept_count;
    assistant_message.tool_calls = Some(original_tool_calls.into_iter().take(kept_count).collect());

    let persisted_messages = build_projected_messages_for_prefix(
        &projection_history,
        assistant_message,
        assistant_message
            .tool_calls
            .as_ref()
            .expect("tool calls preserved"),
        kept_count,
        &synthetic_tool_result,
    );
    let persisted_prompt_tokens = calculate_prompt_anchored_total_tokens(
        &persisted_messages,
        system_prompt_tokens,
        tools_tokens,
    );

    log::warn!(
        "Tool-loop token fence (compaction degraded-tools fallback) redacted assistant tool calls for session {}: kept={} dropped={} projected_prompt_tokens={} projected_total_budget_tokens={} persisted_prompt_tokens={} guarded_total_budget_limit={} output_reserve_tokens={} compact_summary_applied={}",
        session_id,
        kept_count,
        dropped_count,
        kept_projection_prompt,
        kept_projection_total,
        persisted_prompt_tokens,
        guarded_total_budget_limit,
        output_reserve_tokens,
        compact_record.is_some()
    );
}

fn build_projected_messages_for_prefix(
    history_messages: &[Message],
    assistant_message: &Message,
    tool_calls: &[ToolCall],
    prefix_len: usize,
    synthetic_tool_result: &str,
) -> Vec<Message> {
    let kept_tool_calls: Vec<ToolCall> = tool_calls.iter().take(prefix_len).cloned().collect();
    let mut projected_messages =
        Vec::with_capacity(history_messages.len() + 1 + kept_tool_calls.len());
    projected_messages.extend(history_messages.iter().cloned());

    let mut projected_assistant = assistant_message.clone();
    projected_assistant.tool_calls = Some(kept_tool_calls.clone());
    projected_messages.push(projected_assistant);

    projected_messages.extend(kept_tool_calls.iter().map(|tool_call| {
        create_tool_result_message(
            &assistant_message.session_id,
            &tool_call.id,
            synthetic_tool_result.to_string(),
            None,
        )
    }));

    projected_messages
}

fn estimate_parent_request_prefix_tokens(
    parent_request: Option<&crate::agent::llm::types::CompactionParentRequest>,
) -> (usize, usize) {
    let Some(parent_request) = parent_request else {
        return (0, 0);
    };

    let system_prompt_tokens = parent_request
        .system_prompt
        .as_deref()
        .map(estimate_text_tokens)
        .unwrap_or(0)
        + parent_request
            .session_context
            .as_deref()
            .map(estimate_text_tokens)
            .unwrap_or(0);
    let tools_tokens = parent_request
        .available_tools
        .as_ref()
        .map(|tools| estimate_text_tokens(&serde_json::to_string(tools).unwrap_or_default()))
        .unwrap_or(0);

    (system_prompt_tokens, tools_tokens)
}
