use super::ui_break::{
    circuit_break_tool_call, circuit_breaker_action_label, emit_circuit_breaker_triggered,
    ui_alias_enabled,
};
use crate::agent::llm::circuit_breaker;
use crate::agent::llm::natural_recovery::{LoopPreventionKind, LoopPreventionShortCircuit};
use crate::agent::state::AgentSession;
use crate::agent::types::ToolCall;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use crate::repositories::SessionMetadata;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub(super) type HardBreakInfo = (usize, usize, String, String);

#[allow(clippy::too_many_arguments)]
pub(super) async fn collect_loop_prevention_signals(
    active_sessions: &Arc<RwLock<HashMap<String, AgentSession>>>,
    session_id: &str,
    tool_calls: &[ToolCall],
    loop_threshold: usize,
    loop_break_offset: usize,
    wait_settings: &circuit_breaker::WaitPreventionSettings,
    tool_loop_resample_enabled: bool,
    tool_loop_max_resample_retries: usize,
) -> (
    Option<SessionMetadata>,
    Option<HardBreakInfo>,
    HashMap<String, LoopPreventionShortCircuit>,
) {
    let mut loop_prevention_short_circuits = HashMap::new();
    let sessions = active_sessions.read().await;
    let Some(session) = sessions.get(session_id) else {
        return (None, None, loop_prevention_short_circuits);
    };

    let metadata = session.metadata.clone();
    let messages = session.messages.read().await;
    let call_signature_by_id = circuit_breaker::build_tool_call_indices(&messages);
    let mut hard_break = None;

    // Cross-turn identical mixed batches ([a,b,c] → [a,b,c]) are invisible
    // to the per-tool result streak scanner — check batch fingerprints first.
    if let Some(action) = circuit_breaker::evaluate_batch_circuit_breaker(
        &messages,
        tool_calls,
        loop_threshold,
        loop_break_offset,
        wait_settings,
    ) {
        match action {
            circuit_breaker::CircuitBreakerAction::HardBreak {
                count,
                tool_name,
                args,
            } => {
                hard_break = Some((0, count, tool_name, args));
            }
            circuit_breaker::CircuitBreakerAction::RepeatedBatchSequence {
                count,
                tool_name,
                args,
            } => {
                let attempt_index = count.saturating_sub(loop_threshold);
                if tool_loop_resample_enabled && attempt_index >= tool_loop_max_resample_retries {
                    hard_break = Some((0, count, tool_name, args));
                } else {
                    for tool_call in tool_calls.iter() {
                        loop_prevention_short_circuits.insert(
                            tool_call.id.clone(),
                            LoopPreventionShortCircuit {
                                kind: LoopPreventionKind::RepeatedBatchSequence,
                                tool_name: tool_name.clone(),
                                count,
                                args: args.clone(),
                                duplicate_of_tool_call_id: None,
                            },
                        );
                    }
                }
            }
            circuit_breaker::CircuitBreakerAction::RepeatedBatchSequenceEscalate {
                count,
                tool_name,
                args,
            } => {
                let attempt_index = count.saturating_sub(loop_threshold);
                if tool_loop_resample_enabled && attempt_index >= tool_loop_max_resample_retries {
                    hard_break = Some((0, count, tool_name, args));
                } else {
                    for tool_call in tool_calls.iter() {
                        loop_prevention_short_circuits.insert(
                            tool_call.id.clone(),
                            LoopPreventionShortCircuit {
                                kind: LoopPreventionKind::RepeatedBatchSequenceEscalate,
                                tool_name: tool_name.clone(),
                                count,
                                args: args.clone(),
                                duplicate_of_tool_call_id: None,
                            },
                        );
                    }
                }
            }
            // Wait guidance: repeated identical long-running calls.
            // Short-circuit the whole batch with callback guidance; no
            // resample (the wait will look identical again).
            circuit_breaker::CircuitBreakerAction::WaitGuidance {
                count,
                tool_name,
                args,
            } => {
                for tool_call in tool_calls.iter() {
                    loop_prevention_short_circuits.insert(
                        tool_call.id.clone(),
                        LoopPreventionShortCircuit {
                            kind: LoopPreventionKind::WaitGuidance,
                            tool_name: tool_name.clone(),
                            count,
                            args: args.clone(),
                            duplicate_of_tool_call_id: None,
                        },
                    );
                }
            }
            _ => {}
        }
    }

    // A repeated identical batch whose trailing run contains waits is
    // owned by the batch wait ladder — its members must not be
    // fast-blocked per-tool while the wait streak is active (#2610 G1.2).
    if hard_break.is_none()
        && loop_prevention_short_circuits.is_empty()
        && !circuit_breaker::is_wait_batch_streak(&messages, tool_calls, wait_settings)
    {
        let intra_batch_duplicates = circuit_breaker::find_intra_batch_duplicates(tool_calls);

        for (index, tool_call) in tool_calls.iter().enumerate() {
            if let Some(circuit_breaker::CircuitBreakerAction::DuplicateInBatch {
                tool_name,
                args,
                duplicate_of_tool_call_id,
            }) = intra_batch_duplicates.get(&tool_call.id)
            {
                loop_prevention_short_circuits.insert(
                    tool_call.id.clone(),
                    LoopPreventionShortCircuit {
                        kind: LoopPreventionKind::DuplicateInBatch,
                        tool_name: tool_name.clone(),
                        count: 2,
                        args: args.clone(),
                        duplicate_of_tool_call_id: Some(duplicate_of_tool_call_id.clone()),
                    },
                );
                continue;
            }

            let Some(action) = circuit_breaker::evaluate_circuit_breaker_action(
                &messages,
                tool_call,
                &call_signature_by_id,
                loop_threshold,
                loop_break_offset,
                wait_settings,
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
                // Wait guidance: repeated identical long-running calls.
                // Guide to a scheduled callback; no resample — the wait
                // would return identically again.
                circuit_breaker::CircuitBreakerAction::WaitGuidance {
                    count,
                    tool_name,
                    args,
                } => {
                    loop_prevention_short_circuits.insert(
                        tool_call.id.clone(),
                        LoopPreventionShortCircuit {
                            kind: LoopPreventionKind::WaitGuidance,
                            tool_name,
                            count,
                            args,
                            duplicate_of_tool_call_id: None,
                        },
                    );
                }
                circuit_breaker::CircuitBreakerAction::NaturalRecoveryError {
                    count,
                    tool_name,
                    args,
                } => {
                    let attempt_index = count.saturating_sub(loop_threshold);
                    if tool_loop_resample_enabled && attempt_index >= tool_loop_max_resample_retries
                    {
                        hard_break = Some((index, count, tool_name, args));
                        break;
                    }
                    loop_prevention_short_circuits.insert(
                        tool_call.id.clone(),
                        LoopPreventionShortCircuit {
                            kind: LoopPreventionKind::RepeatedErrorOutcome,
                            tool_name,
                            count,
                            args,
                            duplicate_of_tool_call_id: None,
                        },
                    );
                }
                circuit_breaker::CircuitBreakerAction::NaturalRecoveryErrorEscalate {
                    count,
                    tool_name,
                    args,
                } => {
                    let attempt_index = count.saturating_sub(loop_threshold);
                    if tool_loop_resample_enabled && attempt_index >= tool_loop_max_resample_retries
                    {
                        hard_break = Some((index, count, tool_name, args));
                        break;
                    }
                    loop_prevention_short_circuits.insert(
                        tool_call.id.clone(),
                        LoopPreventionShortCircuit {
                            kind: LoopPreventionKind::RepeatedErrorEscalate,
                            tool_name,
                            count,
                            args,
                            duplicate_of_tool_call_id: None,
                        },
                    );
                }
                circuit_breaker::CircuitBreakerAction::NaturalRecoverySuccess {
                    count,
                    tool_name,
                    args,
                } => {
                    let attempt_index = count.saturating_sub(loop_threshold);
                    if tool_loop_resample_enabled && attempt_index >= tool_loop_max_resample_retries
                    {
                        hard_break = Some((index, count, tool_name, args));
                        break;
                    }
                    loop_prevention_short_circuits.insert(
                        tool_call.id.clone(),
                        LoopPreventionShortCircuit {
                            kind: LoopPreventionKind::RepeatedSuccessOutcome,
                            tool_name,
                            count,
                            args,
                            duplicate_of_tool_call_id: None,
                        },
                    );
                }
                _ => {}
            }
        }
    }

    (Some(metadata), hard_break, loop_prevention_short_circuits)
}

pub(super) async fn apply_hard_break(
    session_id: &str,
    session_metadata: Option<&SessionMetadata>,
    assistant_message: &mut Message,
    (index, count, tool_name, args): HardBreakInfo,
) -> Option<MCPContent> {
    let tool_calls = assistant_message.tool_calls.as_mut()?;
    let safe_tool = circuit_breaker::sanitize_circuit_breaker_log_tool_name(&tool_name);
    log::warn!(
        "Circuit breaker triggered for session {} tool {} (count {})",
        session_id,
        safe_tool,
        count
    );
    emit_circuit_breaker_triggered(session_id, &tool_name, count, "hardBreak");

    if ui_alias_enabled(session_metadata, session_id).await {
        tool_calls[index] = circuit_break_tool_call(&tool_name, count, &args, None, None);
        tool_calls.truncate(index + 1);
        None
    } else {
        log::warn!(
            "UI alias disabled for session {}. Using text-only circuit break fallback.",
            session_id
        );

        Some(MCPContent::Text {
            text: format!(
                "⚠️ Circuit breaker triggered: detected runaway loop for tool '{}' (count {}).\n\nThe 'ui' builtin server is disabled for this session, so interactive circuit-break UI was skipped. Workflow was force-stopped to prevent further runaway calls. Review your last attempts and propose a fundamentally different approach.",
                safe_tool, count
            ),
        })
    }
}

pub(super) fn log_and_emit_short_circuits(
    session_id: &str,
    loop_prevention_short_circuits: &HashMap<String, LoopPreventionShortCircuit>,
    emit_events: bool,
) {
    for short_circuit in loop_prevention_short_circuits.values() {
        let safe_tool =
            circuit_breaker::sanitize_circuit_breaker_log_tool_name(&short_circuit.tool_name);
        log::warn!(
            "Loop prevention short-circuit for session {} tool {} (count {}, kind={:?})",
            session_id,
            safe_tool,
            short_circuit.count,
            short_circuit.kind
        );
        if emit_events {
            emit_circuit_breaker_triggered(
                session_id,
                &short_circuit.tool_name,
                short_circuit.count,
                circuit_breaker_action_label(&short_circuit.kind),
            );
        }
    }
}
