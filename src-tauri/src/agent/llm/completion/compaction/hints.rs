use crate::agent::compaction_text::sanitize_compaction_semantic_text;
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use once_cell::sync::Lazy;
use regex::Regex;

use super::instruction::{
    truncate_instruction_text, ACTIVE_REQUEST_BULLET_LIMIT, INSTRUCTION_HINT_TEXT_LIMIT,
    REFERENCE_CONTEXT_WINDOW_MESSAGES, REQUIRED_REFERENCE_BULLET_LIMIT,
};

static BACKTICK_REFERENCE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"`([^`\r\n]+)`").expect("valid backtick regex"));
static PATH_REFERENCE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)\b(?:[a-z]:[\\/]|\.{1,2}[\\/]|[^\\/\s]+[\\/])[\w./\\-]*\.[a-z0-9]{1,12}\b"#)
        .expect("valid path regex")
});
static SYMBOL_REFERENCE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"\b(?:interface|type|class|enum|trait|struct|function|fn|const|let)\s+([A-Za-z_][A-Za-z0-9_]*)",
    )
    .expect("valid symbol regex")
});
static IDENTIFIER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]{1,79}$").expect("valid identifier regex"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionPreservationHints {
    pub active_request: Vec<String>,
    pub required_references: Vec<String>,
}

fn extract_message_text_fragments(message: &Message) -> Vec<String> {
    message
        .content
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            _ => None,
        })
        .collect()
}

fn extract_sanitized_message_text_fragments(message: &Message) -> Vec<String> {
    message
        .content
        .iter()
        .filter_map(|content| match content {
            MCPContent::Text { text, .. } => {
                let sanitized = sanitize_compaction_semantic_text(text);
                let trimmed = sanitized.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            _ => None,
        })
        .collect()
}

fn push_unique_limited(values: &mut Vec<String>, candidate: String, limit: usize) {
    if candidate.is_empty() || values.iter().any(|existing| existing == &candidate) {
        return;
    }

    if values.len() < limit {
        values.push(candidate);
    }
}

fn format_reference_candidate(candidate: &str) -> String {
    if PATH_REFERENCE_RE.is_match(candidate) {
        format!("Preserve file path `{}`", candidate)
    } else {
        format!("Preserve identifier `{}`", candidate)
    }
}

fn is_reference_candidate(text: &str) -> bool {
    PATH_REFERENCE_RE.is_match(text) || IDENTIFIER_RE.is_match(text)
}

fn extract_reference_candidates_from_text(text: &str) -> Vec<String> {
    let mut references = Vec::new();

    for captures in BACKTICK_REFERENCE_RE.captures_iter(text) {
        if let Some(candidate) = captures.get(1).map(|capture| capture.as_str().trim()) {
            if is_reference_candidate(candidate) {
                push_unique_limited(&mut references, candidate.to_string(), usize::MAX);
            }
        }
    }

    for candidate in PATH_REFERENCE_RE
        .find_iter(text)
        .map(|capture| capture.as_str())
    {
        push_unique_limited(&mut references, candidate.to_string(), usize::MAX);
    }

    for captures in SYMBOL_REFERENCE_RE.captures_iter(text) {
        if let Some(candidate) = captures.get(1).map(|capture| capture.as_str()) {
            push_unique_limited(&mut references, candidate.to_string(), usize::MAX);
        }
    }

    references
}

fn extract_compact_summary_body(message: &Message) -> Option<String> {
    if !message.is_compact_summary() {
        return None;
    }

    let text = extract_message_text_fragments(message).join("\n");
    let body = text.strip_prefix("### Previous Conversation Summary\n\n")?;
    let summary_only = body
        .split("\n\n### Recent Tool Call Snapshot (latest 5)\n")
        .next()
        .unwrap_or(body)
        .trim();

    if summary_only.is_empty() {
        None
    } else {
        Some(summary_only.to_string())
    }
}

fn parse_section_heading_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let candidate = if trimmed.starts_with('#') {
        trimmed.trim_start_matches('#').trim()
    } else if !trimmed.starts_with("- ") && !trimmed.starts_with("* ") && trimmed.ends_with(':') {
        trimmed.trim_end_matches(':').trim()
    } else {
        return None;
    };

    if candidate.is_empty() {
        None
    } else {
        Some(candidate.to_string())
    }
}

fn matches_section_heading(line: &str, section_heading: &str) -> bool {
    parse_section_heading_line(line)
        .map(|candidate| candidate == section_heading)
        .unwrap_or(false)
}

pub(super) fn extract_summary_section_bullets(summary: &str, section_heading: &str) -> Vec<String> {
    let mut bullets = Vec::new();
    let mut in_section = false;

    for line in summary.lines() {
        let trimmed = line.trim();

        if matches_section_heading(trimmed, section_heading) {
            in_section = true;
            continue;
        }

        if parse_section_heading_line(trimmed).is_some() {
            in_section = false;
            continue;
        }

        if in_section {
            let bullet = trimmed
                .strip_prefix("- ")
                .or_else(|| trimmed.strip_prefix("* "));
            if let Some(bullet) = bullet {
                let normalized = bullet.trim();
                if !normalized.is_empty() {
                    bullets.push(normalized.to_string());
                }
            }
        }
    }

    bullets
}

fn collect_prior_summary_hints(
    summary_message: &Message,
    active_request: &mut Vec<String>,
    required_references: &mut Vec<String>,
) {
    let Some(summary_body) = extract_compact_summary_body(summary_message) else {
        return;
    };

    for bullet in extract_summary_section_bullets(&summary_body, "Active Request") {
        push_unique_limited(
            active_request,
            truncate_instruction_text(&bullet, INSTRUCTION_HINT_TEXT_LIMIT),
            ACTIVE_REQUEST_BULLET_LIMIT,
        );
    }

    for bullet in extract_summary_section_bullets(&summary_body, "Required References") {
        push_unique_limited(required_references, bullet, REQUIRED_REFERENCE_BULLET_LIMIT);
    }
}

fn collect_reference_candidates_from_json_value(
    value: &serde_json::Value,
    references: &mut Vec<String>,
) {
    match value {
        serde_json::Value::String(text) => {
            let extracted = extract_reference_candidates_from_text(text);
            if extracted.is_empty() && IDENTIFIER_RE.is_match(text) {
                push_unique_limited(references, text.clone(), usize::MAX);
            } else {
                for candidate in extracted {
                    push_unique_limited(references, candidate, usize::MAX);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_reference_candidates_from_json_value(item, references);
            }
        }
        serde_json::Value::Object(map) => {
            for value in map.values() {
                collect_reference_candidates_from_json_value(value, references);
            }
        }
        _ => {}
    }
}

fn collect_reference_candidates_from_message(message: &Message, references: &mut Vec<String>) {
    for text in extract_sanitized_message_text_fragments(message) {
        for candidate in extract_reference_candidates_from_text(&text) {
            push_unique_limited(references, candidate, usize::MAX);
        }
    }

    if let Some(tool_calls) = &message.tool_calls {
        for tool_call in tool_calls {
            if let Ok(arguments) =
                serde_json::from_str::<serde_json::Value>(&tool_call.function.arguments)
            {
                collect_reference_candidates_from_json_value(&arguments, references);
            }
        }
    }
}

pub(super) fn build_compaction_preservation_hints_from_parts(
    prior_summary: Option<&Message>,
    latest_external_request_messages: &[Message],
    reference_context_messages: &[Message],
) -> CompactionPreservationHints {
    let mut active_request = Vec::new();
    let mut required_references = Vec::new();

    for message in latest_external_request_messages {
        for fragment in extract_sanitized_message_text_fragments(message) {
            push_unique_limited(
                &mut active_request,
                truncate_instruction_text(&fragment, INSTRUCTION_HINT_TEXT_LIMIT),
                ACTIVE_REQUEST_BULLET_LIMIT,
            );
            for candidate in extract_reference_candidates_from_text(&fragment) {
                push_unique_limited(
                    &mut required_references,
                    format_reference_candidate(&candidate),
                    REQUIRED_REFERENCE_BULLET_LIMIT,
                );
            }
        }
    }

    let mut raw_reference_candidates = Vec::new();
    for message in reference_context_messages {
        collect_reference_candidates_from_message(message, &mut raw_reference_candidates);
    }

    for candidate in raw_reference_candidates {
        push_unique_limited(
            &mut required_references,
            format_reference_candidate(&candidate),
            REQUIRED_REFERENCE_BULLET_LIMIT,
        );
    }

    if let Some(summary_message) = prior_summary {
        collect_prior_summary_hints(
            summary_message,
            &mut active_request,
            &mut required_references,
        );
    }

    CompactionPreservationHints {
        active_request,
        required_references,
    }
}

pub fn build_compaction_preservation_hints(messages: &[Message]) -> CompactionPreservationHints {
    let prior_summary = messages.iter().find(|message| message.is_compact_summary());
    let (latest_external_request_messages, reference_context_messages) = if let Some((start, end)) =
        super::find_latest_external_request_seed_block_range(messages)
    {
        (
            &messages[start..end],
            &messages[start.saturating_sub(REFERENCE_CONTEXT_WINDOW_MESSAGES)..end],
        )
    } else {
        (&[][..], &[][..])
    };

    build_compaction_preservation_hints_from_parts(
        prior_summary,
        latest_external_request_messages,
        reference_context_messages,
    )
}
