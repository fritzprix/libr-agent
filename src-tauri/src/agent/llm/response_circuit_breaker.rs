use crate::agent::llm::circuit_breaker;
use crate::agent::llm::completion::load_context_management_settings;
use crate::agent::llm::completion::request::apply_compact_summary_projection;
use crate::agent::llm::natural_recovery::{LoopPreventionKind, LoopPreventionShortCircuit};
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

/// Outcome of `preprocess_assistant_tool_calls` after loop detection runs.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CircuitBreakerPreprocessResult {
    pub loop_prevention_short_circuits: HashMap<String, LoopPreventionShortCircuit>,
    pub forced_stop: Option<ForcedCircuitBreakStop>,
}

/// Hard-stop path when repetition exceeds `loopPreventionThreshold`.
#[derive(Debug, PartialEq, Eq)]
pub enum ForcedCircuitBreakStop {
    /// `ui__circuitBreak` was injected into `assistant_message.tool_calls`.
    InteractiveCircuitBreak,
    /// `assistant_message` was converted to text-only forced stop content.
    TextOnly,
}

pub(crate) async fn preprocess_assistant_tool_calls(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    assistant_message: &mut Message,
) -> CircuitBreakerPreprocessResult {
    let mut forced_circuit_break_message = None;
    let mut loop_prevention_short_circuits = HashMap::new();

    if let Some(tool_calls) = &assistant_message.tool_calls {
        let (loop_threshold, loop_break_offset) =
            circuit_breaker::load_loop_prevention_settings().await;

        let (session_metadata, hard_break) = {
            let sessions = active_sessions.read().await;
            match sessions.get(session_id) {
                None => (None, None),
                Some(session) => {
                    let metadata = session.metadata.clone();
                    let messages = session.messages.read().await;
                    let call_signature_by_id = circuit_breaker::build_tool_call_indices(&messages);

                    let mut hard_break = None;
                    for (index, tool_call) in tool_calls.iter().enumerate() {
                        let Some(action) = circuit_breaker::evaluate_circuit_breaker_action(
                            &messages,
                            tool_call,
                            &call_signature_by_id,
                            loop_threshold,
                            loop_break_offset,
                        ) else {
                            continue;
                        };

                        match action {
                            circuit_breaker::CircuitBreakerAction::HardBreak {
                                count,
                                tool_name,
                                args,
                            } => {
                                hard_break = Some((index, count, tool_name, args));
                                break;
                            }
                            circuit_breaker::CircuitBreakerAction::NaturalRecoveryError {
                                count,
                                tool_name,
                                ..
                            } => {
                                loop_prevention_short_circuits.insert(
                                    tool_call.id.clone(),
                                    LoopPreventionShortCircuit {
                                        kind: LoopPreventionKind::RepeatedErrorOutcome,
                                        tool_name,
                                        count,
                                    },
                                );
                            }
                            circuit_breaker::CircuitBreakerAction::NaturalRecoverySuccess {
                                count,
                                tool_name,
                                ..
                            } => {
                                loop_prevention_short_circuits.insert(
                                    tool_call.id.clone(),
                                    LoopPreventionShortCircuit {
                                        kind: LoopPreventionKind::RepeatedSuccessOutcome,
                                        tool_name,
                                        count,
                                    },
                                );
                            }
                        }
                    }

                    (Some(metadata), hard_break)
                }
            }
        };

        if let Some((index, count, tool_name, args)) = hard_break {
            loop_prevention_short_circuits.clear();
            if let Some(tool_calls) = assistant_message.tool_calls.as_mut() {
                log::warn!(
                    "Circuit breaker triggered for session {} tool {} (count {})",
                    session_id,
                    tool_name,
                    count
                );

                let ui_alias_enabled = match session_metadata.as_ref() {
                    Some(metadata) => match crate::agent::resolve_agent_config(metadata).await {
                        Ok(config) => circuit_breaker::is_builtin_alias_enabled(&config, "ui"),
                        Err(error) => {
                            log::warn!(
                                    "Failed to resolve agent config for circuit breaker in session {}: {}",
                                    session_id,
                                    error
                                );
                            true
                        }
                    },
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
                                "⚠️ Circuit breaker triggered: detected runaway loop for tool '{}' (count {}).\n\nThe 'ui' builtin server is disabled for this session, so interactive circuit-break UI was skipped. Workflow was force-stopped to prevent further runaway calls. Review your last attempts and propose a fundamentally different approach.",
                                tool_name, count
                            ),
                            is_error: None,
                        });
                }
            }
        } else if !loop_prevention_short_circuits.is_empty() {
            for short_circuit in loop_prevention_short_circuits.values() {
                log::warn!(
                    "Loop prevention short-circuit for session {} tool {} (count {}, kind={:?})",
                    session_id,
                    short_circuit.tool_name,
                    short_circuit.count,
                    short_circuit.kind
                );
            }
        }
    }

    if let Some(circuit_break_message) = forced_circuit_break_message {
        assistant_message.tool_calls = None;
        assistant_message.content = vec![circuit_break_message.clone()];
        return CircuitBreakerPreprocessResult {
            loop_prevention_short_circuits: HashMap::new(),
            forced_stop: Some(ForcedCircuitBreakStop::TextOnly),
        };
    }

    apply_tool_loop_token_fence(active_sessions, session_id, assistant_message).await;

    let forced_stop = if hard_break_applied_ui_circuit_break(assistant_message) {
        Some(ForcedCircuitBreakStop::InteractiveCircuitBreak)
    } else {
        None
    };

    CircuitBreakerPreprocessResult {
        loop_prevention_short_circuits,
        forced_stop,
    }
}

fn hard_break_applied_ui_circuit_break(assistant_message: &Message) -> bool {
    assistant_message
        .tool_calls
        .as_ref()
        .is_some_and(|tool_calls| {
            tool_calls
                .iter()
                .any(|tool_call| tool_call.function.name == "ui__circuitBreak")
        })
}

pub async fn preprocess_assistant_tool_calls_for_testing(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    assistant_message: &mut Message,
) -> CircuitBreakerPreprocessResult {
    preprocess_assistant_tool_calls(active_sessions, session_id, assistant_message).await
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
