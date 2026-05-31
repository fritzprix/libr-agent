use crate::agent::compaction_text::sanitize_compaction_semantic_text;
use crate::agent::llm::completion::build_compact_summary_message_for_messages;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;

const COMPACT_PREVIEW_MAX_CHARS: usize = 96;
const COMPACTION_SUMMARY_HARD_LIMIT_RATIO: usize = 10;
pub(super) const COMPACTION_SUMMARY_TRUNCATION_SUFFIX: &str = "…";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactSummaryClampResult {
    pub summary: String,
    pub hard_limit_tokens: usize,
    pub estimated_tokens: usize,
    pub original_estimated_tokens: usize,
    pub was_clamped: bool,
}

fn compact_summary_hard_limit_tokens(max_input_context: usize) -> usize {
    std::cmp::max(1, max_input_context / COMPACTION_SUMMARY_HARD_LIMIT_RATIO)
}

fn estimate_wrapped_compact_summary_tokens(
    session_id: &str,
    summary: &str,
    compacted_messages: &[Message],
) -> usize {
    let summary_message =
        build_compact_summary_message_for_messages(session_id, summary, compacted_messages, 0);
    crate::agent::llm::estimate_tokens_bpe(&summary_message)
}

fn truncate_summary_prefix(summary: &str, max_chars: usize) -> String {
    let total_chars = summary.chars().count();
    let prefix = summary.chars().take(max_chars).collect::<String>();
    let trimmed = prefix.trim_end();
    if max_chars < total_chars {
        format!("{}{}", trimmed, COMPACTION_SUMMARY_TRUNCATION_SUFFIX)
    } else {
        trimmed.to_string()
    }
}

pub fn clamp_compact_summary_to_context_limit(
    session_id: &str,
    summary: &str,
    compacted_messages: &[Message],
    max_input_context: usize,
) -> CompactSummaryClampResult {
    let normalized_summary = summary.trim();
    let hard_limit_tokens = compact_summary_hard_limit_tokens(max_input_context);
    let original_estimated_tokens =
        estimate_wrapped_compact_summary_tokens(session_id, normalized_summary, compacted_messages);

    if original_estimated_tokens <= hard_limit_tokens {
        return CompactSummaryClampResult {
            summary: normalized_summary.to_string(),
            hard_limit_tokens,
            estimated_tokens: original_estimated_tokens,
            original_estimated_tokens,
            was_clamped: false,
        };
    }

    let total_chars = normalized_summary.chars().count();
    let mut low = 0usize;
    let mut high = total_chars;
    let mut best_summary = String::new();
    let mut best_estimated_tokens =
        estimate_wrapped_compact_summary_tokens(session_id, &best_summary, compacted_messages);

    while low <= high {
        let mid = low + ((high - low) / 2);
        let candidate = truncate_summary_prefix(normalized_summary, mid);
        let candidate_estimated_tokens =
            estimate_wrapped_compact_summary_tokens(session_id, &candidate, compacted_messages);

        if candidate_estimated_tokens <= hard_limit_tokens {
            best_summary = candidate;
            best_estimated_tokens = candidate_estimated_tokens;
            low = mid + 1;
        } else if mid == 0 {
            break;
        } else {
            high = mid - 1;
        }
    }

    CompactSummaryClampResult {
        summary: best_summary,
        hard_limit_tokens,
        estimated_tokens: best_estimated_tokens,
        original_estimated_tokens,
        was_clamped: true,
    }
}

fn minimum_compact_summary_chars(compacted_delta_count: usize) -> usize {
    match compacted_delta_count {
        0..=2 => 32,
        3..=5 => 64,
        _ => 96,
    }
}

pub(super) fn validate_compact_summary(
    summary: &str,
    compacted_delta_count: usize,
) -> Result<(), String> {
    let normalized = summary.trim();
    if normalized.is_empty() {
        return Err("Compaction summary was empty.".to_string());
    }

    let min_chars = minimum_compact_summary_chars(compacted_delta_count);
    let summary_chars = normalized.chars().count();
    if summary_chars < min_chars {
        return Err(format!(
            "Compaction summary was too short: got {} chars, expected at least {}.",
            summary_chars, min_chars
        ));
    }

    Ok(())
}

pub fn validate_compact_summary_for_testing(
    summary: &str,
    compacted_delta_count: usize,
) -> Result<(), String> {
    validate_compact_summary(summary, compacted_delta_count)
}

fn truncate_preview(value: &str, max_chars: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }

    format!(
        "{}…",
        normalized
            .chars()
            .take(max_chars.saturating_sub(1))
            .collect::<String>()
            .trim_end()
    )
}

fn extract_text_preview(text: &str) -> Option<String> {
    let cleaned = sanitize_compaction_semantic_text(text);
    let preview = truncate_preview(&cleaned, COMPACT_PREVIEW_MAX_CHARS);
    if preview.is_empty() {
        None
    } else {
        Some(preview)
    }
}

pub(super) fn extract_message_preview(message: &Message) -> Option<String> {
    for content in &message.content {
        match content {
            MCPContent::Text { text, .. } => {
                if let Some(preview) = extract_text_preview(text) {
                    return Some(preview);
                }
            }
            MCPContent::Thinking { thinking, .. } => {
                if let Some(preview) = extract_text_preview(thinking) {
                    return Some(preview);
                }
            }
            MCPContent::ToolCall { name, .. } => {
                return Some(truncate_preview(
                    &format!("Tool call: {}", name),
                    COMPACT_PREVIEW_MAX_CHARS,
                ));
            }
            _ => {}
        }
    }

    if let Some(tool_calls) = &message.tool_calls {
        if let Some(tool_call) = tool_calls.first() {
            return Some(truncate_preview(
                &format!("Tool call: {}", tool_call.function.name),
                COMPACT_PREVIEW_MAX_CHARS,
            ));
        }
    }

    if message.role == "tool" {
        return Some("Tool result".to_string());
    }

    None
}
