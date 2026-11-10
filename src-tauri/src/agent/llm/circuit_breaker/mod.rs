//! Tool-call loop prevention: decides when repeated identical calls or batches get
//! natural-recovery guidance, wait guidance, or a hard break.

mod outcome_signature;
mod settings;
mod signature;
mod streak;

use crate::agent::types::ToolCall;
use crate::agent::AgentConfig;
use crate::models::chat::Message;
use outcome_signature::RepeatedOutcome;
pub(crate) use settings::load_loop_prevention_settings;
pub use settings::WaitPreventionSettings;
pub use signature::{
    batch_fingerprint, build_tool_call_indices, normalize_tool_arguments,
    sanitize_circuit_breaker_log_tool_name, tool_call_signature,
    TRUNCATED_OR_MALFORMED_ARGS_SIGNATURE,
};
pub use streak::count_consecutive_identical_batches;
use streak::{batch_wait_ladder_owns, count_consecutive_identical_call_outcomes};

#[derive(Debug, PartialEq)]
pub enum CircuitBreakerAction {
    NaturalRecoveryError {
        count: usize,
        tool_name: String,
        args: String,
    },
    /// Pre-hard-break escalation for repeated identical errors: nudge strategy reset.
    NaturalRecoveryErrorEscalate {
        count: usize,
        tool_name: String,
        args: String,
    },
    NaturalRecoverySuccess {
        count: usize,
        tool_name: String,
        args: String,
    },
    /// Same (name, args) already present earlier in the current tool_calls batch.
    DuplicateInBatch {
        tool_name: String,
        args: String,
        /// First tool_call_id in this batch that already used the same signature.
        duplicate_of_tool_call_id: String,
    },
    /// Entire assistant tool_calls batch fingerprint repeated across turns.
    RepeatedBatchSequence {
        count: usize,
        tool_name: String,
        args: String,
    },
    /// Pre-hard-break escalation for repeated identical batches.
    RepeatedBatchSequenceEscalate {
        count: usize,
        tool_name: String,
        args: String,
    },
    /// Identical long-running calls: the model is waiting, not looping. Guide it to a
    /// scheduled callback instead of repeating the wait; hard-break if it keeps polling
    /// past the guidance (#2610).
    WaitGuidance {
        count: usize,
        tool_name: String,
        args: String,
    },
    HardBreak {
        count: usize,
        tool_name: String,
        args: String,
    },
}

pub(crate) fn is_builtin_alias_enabled(agent_config: &AgentConfig, alias: &str) -> bool {
    crate::agent::tools::is_builtin_service_alias_enabled(agent_config, alias)
}

/// Overall identical-(call, outcome) streak cap (#2610 G3.1, G4.1).
///
/// The wait ladder sees only its newest slow run and the fast ladder only its
/// newest fast run, so a duration mix could restart both forever. The cap
/// bounds the total streak. It is never lower than the fast ladder's own hard
/// break, so a pure fast streak always follows the configured loop-prevention
/// thresholds, whatever the wait settings are.
fn overall_streak_cap(
    threshold: usize,
    hard_break_offset: usize,
    wait: &WaitPreventionSettings,
) -> usize {
    (wait.guide_count + wait.break_count).max(threshold + hard_break_offset)
}

/// Collect later-in-batch duplicates of an earlier (name, args) signature.
///
/// Signatures are computed once per call (O(n)). The first occurrence of each
/// signature is remembered by tool_call_id so guidance can name the original.
pub fn find_intra_batch_duplicates(
    tool_calls: &[ToolCall],
) -> std::collections::HashMap<String, CircuitBreakerAction> {
    let mut first_id_by_signature = std::collections::HashMap::new();
    let mut duplicates = std::collections::HashMap::new();

    for tool_call in tool_calls {
        let signature = tool_call_signature(tool_call);
        match first_id_by_signature.entry(signature) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(tool_call.id.clone());
            }
            std::collections::hash_map::Entry::Occupied(entry) => {
                duplicates.insert(
                    tool_call.id.clone(),
                    CircuitBreakerAction::DuplicateInBatch {
                        tool_name: tool_call.function.name.clone(),
                        args: tool_call.function.arguments.clone(),
                        duplicate_of_tool_call_id: entry.get().clone(),
                    },
                );
            }
        }
    }

    duplicates
}

/// Detect repeated identical tool_calls batches across consecutive turns.
///
/// Triggers when both the structural fingerprint (ordered name+args) and the
/// trailing complete batch outcome signature repeat. Progressive polling that
/// changes tool results resets the streak. When triggered, every call in the
/// batch is part of the loop — the preprocess layer short-circuits the whole
/// batch (not a subset).
pub fn evaluate_batch_circuit_breaker(
    messages: &[Message],
    tool_calls: &[ToolCall],
    threshold: usize,
    hard_break_offset: usize,
    wait: &WaitPreventionSettings,
) -> Option<CircuitBreakerAction> {
    if tool_calls.is_empty() {
        return None;
    }

    // Single-tool batches are already covered by per-call streak scanning.
    if tool_calls.len() < 2 {
        return None;
    }

    let first = &tool_calls[0];
    // Exempt recovery/stop tools so the escalate→reflect path and hard-break
    // injection are not themselves treated as looping calls.
    if first.function.name == "ui__circuitBreak" || first.function.name == "planning__reflect" {
        return None;
    }

    let fingerprint = batch_fingerprint(tool_calls);
    let (previous, slow_suffix, fast_suffix) = count_consecutive_identical_batches(
        messages,
        &fingerprint,
        wait,
        threshold + hard_break_offset,
    );
    if previous == 0 {
        return None;
    }

    // Overall cap (#2610 G3.1, G4.1): bounds duration-mixed streaks without
    // ever firing before the fast ladder's configured hard break.
    let overall_total = previous + 1;
    if overall_total > overall_streak_cap(threshold, hard_break_offset, wait) {
        return Some(CircuitBreakerAction::HardBreak {
            count: overall_total,
            tool_name: first.function.name.clone(),
            args: first.function.arguments.clone(),
        });
    }

    // Wait ladder for repeated identical wait batches (see evaluate_circuit_breaker_action).
    if slow_suffix > 0 {
        let wait_total = slow_suffix + 1;
        if wait_total > wait.guide_count + wait.break_count {
            return Some(CircuitBreakerAction::HardBreak {
                count: wait_total,
                tool_name: first.function.name.clone(),
                args: first.function.arguments.clone(),
            });
        }
        if wait_total > wait.guide_count {
            return Some(CircuitBreakerAction::WaitGuidance {
                count: wait_total,
                tool_name: first.function.name.clone(),
                args: first.function.arguments.clone(),
            });
        }
        return None;
    }

    // Fast ladder: counts only the trailing run of non-wait batches (#2610 G1.3).
    let total_count = fast_suffix + 1;
    let hard_break_at = threshold + hard_break_offset;

    if total_count >= hard_break_at {
        Some(CircuitBreakerAction::HardBreak {
            count: total_count,
            tool_name: first.function.name.clone(),
            args: first.function.arguments.clone(),
        })
    } else {
        let pre_hard_count = hard_break_at.saturating_sub(1);
        if total_count > threshold && total_count == pre_hard_count && hard_break_offset >= 2 {
            Some(CircuitBreakerAction::RepeatedBatchSequenceEscalate {
                count: total_count,
                tool_name: first.function.name.clone(),
                args: first.function.arguments.clone(),
            })
        } else if total_count >= threshold {
            Some(CircuitBreakerAction::RepeatedBatchSequence {
                count: total_count,
                tool_name: first.function.name.clone(),
                args: first.function.arguments.clone(),
            })
        } else {
            None
        }
    }
}

/// Whether the current batch continues a repeated identical batch whose
/// trailing run contains waits (#2610 G1.2).
///
/// When true, the batch **wait ladder owns every member** of the batch:
/// even a fast member (e.g. `agent__checkSession` next to a slow
/// `waitForProcess`) must not fall through to the per-tool fast ladder,
/// or the two ladders would disagree on the same repeated wait batch.
pub fn is_wait_batch_streak(
    messages: &[Message],
    tool_calls: &[ToolCall],
    wait: &WaitPreventionSettings,
) -> bool {
    // Mirror `evaluate_batch_circuit_breaker`'s exclusions exactly.
    if tool_calls.len() < 2 {
        return false;
    }
    let first = &tool_calls[0];
    if first.function.name == "ui__circuitBreak" || first.function.name == "planning__reflect" {
        return false;
    }

    let fingerprint = batch_fingerprint(tool_calls);
    let (previous, slow_suffix, _) =
        count_consecutive_identical_batches(messages, &fingerprint, wait, 0);
    batch_wait_ladder_owns(previous, slow_suffix)
}

pub fn evaluate_circuit_breaker_action(
    messages: &[Message],
    tool_call: &ToolCall,
    call_signature_by_id: &std::collections::HashMap<String, String>,
    threshold: usize,
    hard_break_offset: usize,
    wait: &WaitPreventionSettings,
) -> Option<CircuitBreakerAction> {
    let tool_name = &tool_call.function.name;
    let args = &tool_call.function.arguments;

    // Exempt recovery/stop tools so the escalate→reflect path and hard-break
    // injection are not themselves treated as looping calls.
    if tool_name == "ui__circuitBreak" || tool_name == "planning__reflect" {
        return None;
    }

    let current_signature = tool_call_signature(tool_call);
    if let Some((slow_suffix, fast_suffix, total_matches, outcome)) =
        count_consecutive_identical_call_outcomes(
            messages,
            |tool_call_id| call_signature_by_id.get(tool_call_id) == Some(&current_signature),
            wait,
        )
    {
        // Overall cap (#2610 G3.1, G4.1): bounds duration-mixed streaks
        // (alternating slow/fast, or 9 slow + 1 fast repeating) without ever
        // firing before the fast ladder's configured hard break.
        let overall_total = total_matches + 1;
        if overall_total > overall_streak_cap(threshold, hard_break_offset, wait) {
            return Some(CircuitBreakerAction::HardBreak {
                count: overall_total,
                tool_name: tool_name.clone(),
                args: args.clone(),
            });
        }

        // Wait ladder: identical calls that each took a long time are the model
        // *waiting* for background work, not fast-looping. Snapshot polls (fast,
        // identical output) never enter here and keep using the fast rule.
        if slow_suffix > 0 {
            let wait_total = slow_suffix + 1;
            if wait_total > wait.guide_count + wait.break_count {
                return Some(CircuitBreakerAction::HardBreak {
                    count: wait_total,
                    tool_name: tool_name.clone(),
                    args: args.clone(),
                });
            }
            if wait_total > wait.guide_count {
                return Some(CircuitBreakerAction::WaitGuidance {
                    count: wait_total,
                    tool_name: tool_name.clone(),
                    args: args.clone(),
                });
            }
            return None;
        }

        // Fast ladder: counts only the trailing run of fast identical results.
        // Attempts that were part of a wait streak (e.g. N slow waits followed
        // by 1 fast identical result) must not inflate the fast count (#2610 G1.3).
        let total_count = fast_suffix + 1;
        let hard_break_at = threshold + hard_break_offset;

        if total_count >= hard_break_at {
            return Some(CircuitBreakerAction::HardBreak {
                count: total_count,
                tool_name: tool_name.clone(),
                args: args.clone(),
            });
        }

        // NaturalRecoveryErrorEscalate fires strictly between NaturalRecoveryError and HardBreak.
        // Guard: total_count > threshold ensures it cannot fire at the same count as
        // NaturalRecoveryError (which would silently suppress the soft warning).
        // With threshold=3, offset=1: hard_break_at=4, pre_hard_count=3 == threshold →
        // Escalate guard fails → soft fires. With offset>=2 the gap exists.
        let pre_hard_count = hard_break_at.saturating_sub(1);
        if matches!(outcome, RepeatedOutcome::Error { .. })
            && total_count > threshold
            && total_count == pre_hard_count
        {
            return Some(CircuitBreakerAction::NaturalRecoveryErrorEscalate {
                count: total_count,
                tool_name: tool_name.clone(),
                args: args.clone(),
            });
        }

        // Soft-block every identical-(call, outcome) attempt from threshold until hard break.
        // Using `== threshold` would allow retries in the offset gap (e.g. counts 4–5 when
        // threshold=3 and offset=3), which defeats loop prevention.
        if total_count >= threshold {
            return Some(match outcome {
                RepeatedOutcome::Error { .. } => CircuitBreakerAction::NaturalRecoveryError {
                    count: total_count,
                    tool_name: tool_name.clone(),
                    args: args.clone(),
                },
                RepeatedOutcome::Success { .. } => CircuitBreakerAction::NaturalRecoverySuccess {
                    count: total_count,
                    tool_name: tool_name.clone(),
                    args: args.clone(),
                },
            });
        }
    }

    None
}
