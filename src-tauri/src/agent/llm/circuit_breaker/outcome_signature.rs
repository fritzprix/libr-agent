//! Outcome fingerprints of tool results, so streaks compare (call, outcome) pairs.

use crate::agent::types::ToolCall;
use crate::models::chat::Message;
use once_cell::sync::Lazy;
use regex::{Captures, Regex};

/// Shell / interactive success (and failure) headers embed wall-clock duration; spill
/// notices embed per-call UUIDs. Both must be stabilized so RepeatedSuccessOutcome
/// streaks can match semantically identical successes (#1776).
static OUTCOME_COMMAND_DURATION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)((?:Interactive )?command (?:executed|failed) in )(?:<\s*1|\d+)\s*ms")
        .expect("valid command-duration outcome regex")
});
static OUTCOME_TOOL_RESULT_SPILL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\.libragent/tool-results/[A-Za-z0-9._-]+\.txt")
        .expect("valid tool-result spill path outcome regex")
});

pub(super) fn is_tool_error_message(message: &Message) -> bool {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RepeatedOutcome {
    Success { signature: String },
    Error { signature: String },
}

fn canonical_structured_field(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn extract_structured_loop_fingerprint(message: &Message) -> Option<String> {
    let structured = message.metadata.as_ref()?.get("structuredContent")?;

    if let Some(fingerprint) = structured
        .get("loopFingerprint")
        .and_then(|value| value.as_str())
    {
        let trimmed = fingerprint.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    // Shell / runCommand payloads carry `duration_ms` and a coarse `status`
    // (finished|failed). Prefer stabilized text so ephemeral duration/spill
    // paths participate in the fingerprint, and so distinct stdout still breaks
    // streaks (unlike bare status=finished).
    if structured.get("duration_ms").is_some() {
        return None;
    }

    let mut parts = Vec::new();
    for key in [
        "status",
        "turnCount",
        "responseStatus",
        "process_id",
        "processId",
    ] {
        if let Some(value) = structured.get(key) {
            parts.push(format!("{key}={}", canonical_structured_field(value)));
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("|"))
    }
}

/// Strip ephemeral per-call metadata from tool-result text before fingerprinting.
///
/// Does not change what the model sees — only the loop-prevention outcome signature.
fn stabilize_ephemeral_outcome_text(text: &str) -> String {
    let without_duration = OUTCOME_COMMAND_DURATION_RE
        .replace_all(text, |caps: &Captures| format!("{}<duration>", &caps[1]));
    OUTCOME_TOOL_RESULT_SPILL_RE
        .replace_all(&without_duration, ".libragent/tool-results/<spill>.txt")
        .into_owned()
}

fn normalize_text_signature(text: &str) -> String {
    stabilize_ephemeral_outcome_text(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn build_tool_result_signature(message: &Message) -> String {
    if let Some(fingerprint) = extract_structured_loop_fingerprint(message) {
        return fingerprint;
    }

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

pub(super) fn is_loop_prevention_message(message: &Message) -> bool {
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

pub(super) fn repeated_outcome_key(outcome: &RepeatedOutcome) -> (&str, bool) {
    match outcome {
        RepeatedOutcome::Success { signature } => (signature.as_str(), false),
        RepeatedOutcome::Error { signature } => (signature.as_str(), true),
    }
}

/// Build a stable outcome fingerprint for every tool in a batch.
///
/// Returns `None` when any result is **missing** or is a **loop-prevention**
/// short-circuit. Callers must treat `None` as "preserve call-fingerprint streak
/// without comparing outcomes" — not as a shared empty signature. Two incomplete
/// turns with the same call fingerprint therefore continue the streak (intentional:
/// Soft/Hard ladders must not reset when a prior Soft result or partial tool set
/// is present). Complete divergent outcomes still break the streak via `Some(...)`.
pub(super) fn build_batch_outcome_signature(
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

/// Recorded wall-clock execution duration (ms) on a persisted tool-result message.
///
/// Written by `handle_tool_result` as `metadata.executionDurationMs` for every
/// tool — builtin and external MCP alike — so wait detection needs no per-tool
/// knowledge (#2610).
pub(super) fn tool_result_duration_ms(message: &Message) -> Option<u64> {
    message
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("executionDurationMs"))
        .and_then(|value| value.as_u64())
}
