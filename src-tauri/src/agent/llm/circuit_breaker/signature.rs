//! Pure call and batch signatures: canonical arguments, per-call and per-batch
//! fingerprints, and log-safe tool names.

use crate::agent::types::ToolCall;
use crate::models::chat::Message;
use serde_json::Value;
use std::collections::BTreeMap;

pub const TRUNCATED_OR_MALFORMED_ARGS_SIGNATURE: &str = "<truncated_or_malformed_args>";

/// Canonicalize tool arguments so key-order differences do not evade signatures.
///
/// `serde_json::from_str` already rejects nesting deeper than 128; we still pass an
/// explicit depth budget as defense-in-depth for any future Value sources.
pub fn normalize_tool_arguments(args: &str) -> String {
    if args.trim().is_empty() {
        return "{}".to_string();
    }
    if crate::agent::llm::tool_args_validation::is_redacted_marker(args) {
        return TRUNCATED_OR_MALFORMED_ARGS_SIGNATURE.to_string();
    }
    match serde_json::from_str::<Value>(args) {
        Ok(value) => canonical_json_value(&value, 0),
        Err(_) => TRUNCATED_OR_MALFORMED_ARGS_SIGNATURE.to_string(),
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
        Value::String(s) => serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string()),
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
