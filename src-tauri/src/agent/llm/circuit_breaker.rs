use crate::agent::types::ToolCall;
use crate::agent::AgentConfig;
use crate::models::chat::Message;
use crate::repositories::settings_repository::SettingsRepository;
use serde_json::Value;
use std::collections::BTreeMap;

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
    DuplicateInBatch { tool_name: String, args: String },
    /// Entire assistant tool_calls batch fingerprint repeated across turns.
    RepeatedBatchSequence {
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

fn is_tool_error_message(message: &Message) -> bool {
    if message.role != "tool" {
        return false;
    }

    let metadata_tool_error = message
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("toolError"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    if metadata_tool_error {
        return true;
    }

    false
}

/// Canonicalize tool arguments so key-order differences do not evade signatures.
///
/// `serde_json::from_str` already rejects nesting deeper than 128; we still pass an
/// explicit depth budget as defense-in-depth for any future Value sources.
pub fn normalize_tool_arguments(args: &str) -> String {
    match serde_json::from_str::<Value>(args) {
        Ok(value) => canonical_json_value(&value, 0),
        Err(_) => args.to_string(),
    }
}

/// Matches `serde_json`'s default recursion limit so we never walk deeper than the parser allows.
const CANONICAL_JSON_MAX_DEPTH: usize = 128;

/// Soft cap for batch fingerprints. Larger batches collapse to a length-tagged hash so
/// pathological agent outputs cannot amplify memory while identical batches still match.
const MAX_BATCH_FINGERPRINT_BYTES: usize = 64 * 1024;

fn canonical_json_value(value: &Value, depth: usize) -> String {
    if depth >= CANONICAL_JSON_MAX_DEPTH {
        return "\"__max_depth__\"".to_string();
    }

    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<&str, String> = map
                .iter()
                .map(|(key, child)| (key.as_str(), canonical_json_value(child, depth + 1)))
                .collect();
            let body = sorted
                .into_iter()
                .map(|(key, child)| format!("\"{}\":{}", key, child))
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{}}}", body)
        }
        Value::Array(items) => {
            let body = items
                .iter()
                .map(|child| canonical_json_value(child, depth + 1))
                .collect::<Vec<_>>()
                .join(",");
            format!("[{}]", body)
        }
        other => other.to_string(),
    }
}

pub fn tool_call_signature(tool_call: &ToolCall) -> String {
    format!(
        "{}:{}",
        tool_call.function.name,
        normalize_tool_arguments(&tool_call.function.arguments)
    )
}

/// Strip control characters and truncate labels before writing them to logs.
pub fn sanitize_circuit_breaker_log_tool_name(tool_name: &str) -> String {
    let sanitized: String = tool_name
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .take(200)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if sanitized.is_empty() {
        "<unknown-tool>".to_string()
    } else {
        sanitized
    }
}

/// Ordered fingerprint of an assistant tool_calls batch (name+args per call).
pub fn batch_fingerprint(tool_calls: &[ToolCall]) -> String {
    let full = tool_calls
        .iter()
        .map(tool_call_signature)
        .collect::<Vec<_>>()
        .join("\n");

    if full.len() <= MAX_BATCH_FINGERPRINT_BYTES {
        return full;
    }

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    full.hash(&mut hasher);
    format!("hashed:{:016x}:len={}", hasher.finish(), full.len())
}

/// Build signature_by_id lookup map from message history in a single pass.
pub fn build_tool_call_indices(messages: &[Message]) -> std::collections::HashMap<String, String> {
    let mut call_signature_by_id = std::collections::HashMap::new();

    for message in messages {
        if let Some(tool_calls) = &message.tool_calls {
            for tool_call in tool_calls {
                call_signature_by_id.insert(tool_call.id.clone(), tool_call_signature(tool_call));
            }
        }
    }

    call_signature_by_id
}

/// Load loop-prevention thresholds from advanced settings.
///
/// Returns `(threshold, hard_break_offset)`:
/// - `threshold`: consecutive identical **(call, outcome)** streak length that
///   triggers soft natural recovery (default 3). Outcome text changes reset the
///   streak for both per-tool and multi-tool batch scanners.
/// - `hard_break_offset`: added to `threshold` for hard break (default **2**).
///   Soft blocks every count in `[threshold, hard_break)`. For repeated **errors**,
///   `NaturalRecoveryErrorEscalate` fires once at `hard_break - 1` when
///   `offset >= 2` (with offset=1 the escalate step is skipped: soft then hard).
pub(crate) async fn load_loop_prevention_settings() -> (usize, usize) {
    let default_threshold = 3;
    let default_offset = 2;
    let Some(settings_repo) = crate::state::try_get_settings_repository() else {
        return (default_threshold, default_offset);
    };

    match settings_repo.get("advancedSettings").await {
        Ok(Some(model)) => match serde_json::from_str::<serde_json::Value>(&model.value) {
            Ok(json) => {
                let threshold = json
                    .get("loopPreventionThreshold")
                    .and_then(|value| value.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(default_threshold)
                    .clamp(2, 20);
                let offset = json
                    .get("loopPreventionHardBreakOffset")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(default_offset)
                    .clamp(1, 20);
                (threshold, offset)
            }
            Err(_) => (default_threshold, default_offset),
        },
        _ => (default_threshold, default_offset),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RepeatedOutcome {
    Success { signature: String },
    Error { signature: String },
}

fn normalize_text_signature(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn build_tool_result_signature(message: &Message) -> String {
    let text_signature = message
        .content
        .iter()
        .filter_map(|content| match content {
            crate::mcp::types::MCPContent::Text { text, .. } => {
                let normalized = normalize_text_signature(text);
                if normalized.is_empty() {
                    None
                } else {
                    Some(normalized)
                }
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    if text_signature.is_empty() {
        if is_tool_error_message(message) {
            "__tool_error__".to_string()
        } else {
            "__tool_success__".to_string()
        }
    } else {
        text_signature
    }
}

fn is_loop_prevention_message(message: &Message) -> bool {
    if message
        .metadata
        .as_ref()
        .and_then(|metadata| {
            metadata
                .get("loopPrevention")
                .or_else(|| {
                    metadata
                        .get("structuredContent")
                        .and_then(|value| value.get("loopPrevention"))
                })
                .and_then(|value| value.as_bool())
        })
        .unwrap_or(false)
    {
        return true;
    }

    message.content.iter().any(|content| {
        matches!(
            content,
            crate::mcp::types::MCPContent::Text { text, .. }
                if text.starts_with("Loop prevention:")
        )
    })
}

fn repeated_outcome_key(outcome: &RepeatedOutcome) -> (&str, bool) {
    match outcome {
        RepeatedOutcome::Success { signature } => (signature.as_str(), false),
        RepeatedOutcome::Error { signature } => (signature.as_str(), true),
    }
}

/// Trailing assistant tool-call turns, newest first.
///
/// Stops at the first non-tool / non-tool-calling-assistant message (e.g. user).
struct ToolCallTurn<'a> {
    assistant_message: &'a Message,
    tool_results: Vec<&'a Message>,
}

fn group_trailing_tool_call_turns(messages: &[Message]) -> Vec<ToolCallTurn<'_>> {
    let mut turns: Vec<ToolCallTurn> = Vec::new();
    let mut current_tool_results: Vec<&Message> = Vec::new();

    for message in messages.iter().rev() {
        match message.role.as_str() {
            "tool" => {
                current_tool_results.push(message);
            }
            "assistant" if message.tool_calls.is_some() => {
                turns.push(ToolCallTurn {
                    assistant_message: message,
                    tool_results: std::mem::take(&mut current_tool_results),
                });
            }
            _ => {
                break;
            }
        }
    }

    turns
}

/// Build a stable outcome fingerprint for every tool in a batch.
///
/// Returns `None` when any result is **missing** or is a **loop-prevention**
/// short-circuit. Callers must treat `None` as "preserve call-fingerprint streak
/// without comparing outcomes" — not as a shared empty signature. Two incomplete
/// turns with the same call fingerprint therefore continue the streak (intentional:
/// Soft/Hard ladders must not reset when a prior Soft result or partial tool set
/// is present). Complete divergent outcomes still break the streak via `Some(...)`.
fn build_batch_outcome_signature(
    tool_calls: &[ToolCall],
    tool_results: &[&Message],
) -> Option<String> {
    let mut parts = Vec::with_capacity(tool_calls.len());
    for tool_call in tool_calls {
        let tool_result_msg = tool_results
            .iter()
            .find(|message| message.tool_call_id.as_deref() == Some(&tool_call.id))?;
        if is_loop_prevention_message(tool_result_msg) {
            return None;
        }
        let signature = build_tool_result_signature(tool_result_msg);
        let kind = if is_tool_error_message(tool_result_msg) {
            'E'
        } else {
            'S'
        };
        parts.push(format!("{kind}:{signature}"));
    }
    Some(parts.join("\n"))
}

/// Count trailing tool results whose call signature matches `matcher`.
///
/// Counts only the trailing streak of identical (call, outcome) pairs:
/// - A different tool call (name/args) ends the streak.
/// - A different tool-result signature also ends the streak (allows legitimate
///   polling where status progresses, e.g. running → completed).
/// - Loop-prevention short-circuit results keep the streak (they must not look
///   like a “new outcome” that clears the counter).
fn count_consecutive_identical_call_outcomes<F>(
    messages: &[Message],
    matcher: F,
) -> Option<(usize, RepeatedOutcome)>
where
    F: Fn(&str) -> bool,
{
    let turns = group_trailing_tool_call_turns(messages);

    let mut consecutive_matches = 0;
    let mut repeated_outcome: Option<RepeatedOutcome> = None;

    for turn in &turns {
        let mut matched_in_turn = false;
        let mut turn_outcome: Option<RepeatedOutcome> = None;

        if let Some(tool_calls) = &turn.assistant_message.tool_calls {
            for tool_call in tool_calls {
                if matcher(&tool_call.id) {
                    matched_in_turn = true;
                    if let Some(tool_result_msg) = turn
                        .tool_results
                        .iter()
                        .find(|m| m.tool_call_id.as_deref() == Some(&tool_call.id))
                    {
                        if !is_loop_prevention_message(tool_result_msg) {
                            let signature = build_tool_result_signature(tool_result_msg);
                            turn_outcome = Some(if is_tool_error_message(tool_result_msg) {
                                RepeatedOutcome::Error { signature }
                            } else {
                                RepeatedOutcome::Success { signature }
                            });
                        }
                    }
                }
            }
        }

        if !matched_in_turn {
            break;
        }

        if let Some(outcome) = turn_outcome {
            match &repeated_outcome {
                None => {
                    repeated_outcome = Some(outcome);
                    consecutive_matches += 1;
                }
                Some(prev) if repeated_outcome_key(prev) == repeated_outcome_key(&outcome) => {
                    consecutive_matches += 1;
                }
                Some(_) => {
                    // Outcome changed — stop at the trailing identical-outcome segment.
                    break;
                }
            }
        } else {
            // Loop-prevention or missing result: keep the streak alive.
            consecutive_matches += 1;
        }
    }

    if consecutive_matches == 0 {
        return None;
    }

    let outcome = repeated_outcome.unwrap_or_else(|| RepeatedOutcome::Error {
        signature: "__loop_prevention__".to_string(),
    });
    Some((consecutive_matches, outcome))
}

/// Maximum trailing assistant tool_calls batches to inspect when computing a
/// batch-repetition streak. Threshold is clamped to ≤20, so this is ample and
/// bounds work on very long sessions. User/system messages still reset the streak.
const MAX_ASSISTANT_BATCHES_TO_SCAN: usize = 32;

/// Count consecutive prior assistant batches whose fingerprint matches `fingerprint`
/// **and** whose complete tool-result outcomes match the trailing outcome segment.
///
/// Intervening incomplete or loop-prevention results keep the streak. A different
/// non-empty tool_calls batch, a different complete batch outcome, or a user/system
/// message ends the streak. This catches `[a,b,c] → [a,b,c]` which the per-tool
/// result scan misses, while allowing progressive polling across repeated batches.
pub fn count_consecutive_identical_batches(messages: &[Message], fingerprint: &str) -> usize {
    let turns = group_trailing_tool_call_turns(messages);
    let mut consecutive = 0;
    let mut repeated_batch_outcome: Option<String> = None;
    let mut assistant_batches_scanned = 0;

    for turn in &turns {
        let Some(tool_calls) = turn.assistant_message.tool_calls.as_ref() else {
            break;
        };
        if tool_calls.is_empty() {
            break;
        }

        assistant_batches_scanned += 1;
        if assistant_batches_scanned > MAX_ASSISTANT_BATCHES_TO_SCAN {
            break;
        }

        if batch_fingerprint(tool_calls) != fingerprint {
            break;
        }

        if let Some(outcome) = build_batch_outcome_signature(tool_calls, &turn.tool_results) {
            match &repeated_batch_outcome {
                None => {
                    repeated_batch_outcome = Some(outcome);
                    consecutive += 1;
                }
                Some(prev) if prev == &outcome => {
                    consecutive += 1;
                }
                Some(_) => {
                    break;
                }
            }
        } else {
            // Incomplete results or loop-prevention: keep the streak.
            consecutive += 1;
        }
    }

    consecutive
}

/// Collect later-in-batch duplicates of an earlier (name, args) signature.
///
/// Signatures are computed once per call (O(n)), then checked with a set.
pub fn find_intra_batch_duplicates(
    tool_calls: &[ToolCall],
) -> std::collections::HashMap<String, CircuitBreakerAction> {
    let mut seen_signatures = std::collections::HashSet::new();
    let mut duplicates = std::collections::HashMap::new();

    for tool_call in tool_calls {
        let signature = tool_call_signature(tool_call);
        if !seen_signatures.insert(signature) {
            duplicates.insert(
                tool_call.id.clone(),
                CircuitBreakerAction::DuplicateInBatch {
                    tool_name: tool_call.function.name.clone(),
                    args: tool_call.function.arguments.clone(),
                },
            );
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
    let previous = count_consecutive_identical_batches(messages, &fingerprint);
    if previous == 0 {
        return None;
    }

    let total_count = previous + 1;
    let hard_break_at = threshold + hard_break_offset;

    if total_count >= hard_break_at {
        Some(CircuitBreakerAction::HardBreak {
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

pub fn evaluate_circuit_breaker_action(
    messages: &[Message],
    tool_call: &ToolCall,
    call_signature_by_id: &std::collections::HashMap<String, String>,
    threshold: usize,
    hard_break_offset: usize,
) -> Option<CircuitBreakerAction> {
    let tool_name = &tool_call.function.name;
    let args = &tool_call.function.arguments;

    // Exempt recovery/stop tools so the escalate→reflect path and hard-break
    // injection are not themselves treated as looping calls.
    if tool_name == "ui__circuitBreak" || tool_name == "planning__reflect" {
        return None;
    }

    let current_signature = tool_call_signature(tool_call);
    if let Some((consecutive_identical_signature, outcome)) =
        count_consecutive_identical_call_outcomes(messages, |tool_call_id| {
            call_signature_by_id.get(tool_call_id) == Some(&current_signature)
        })
    {
        let total_count = consecutive_identical_signature + 1;
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
