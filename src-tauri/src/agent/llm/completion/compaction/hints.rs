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

fn extract_summary_section_bullets(summary: &str, section_heading: &str) -> Vec<String> {
    let mut bullets = Vec::new();
    let mut in_section = false;
    let markdown_heading = format!("### {}", section_heading);

    for line in summary.lines() {
        let trimmed = line.trim();

        if trimmed == markdown_heading || trimmed == section_heading {
            in_section = true;
            continue;
        }

        if trimmed.starts_with("### ") || trimmed.ends_with(':') {
            in_section = false;
        }

        if in_section {
            if let Some(bullet) = trimmed.strip_prefix("- ") {
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
    message: &Message,
    active_request: &mut Vec<String>,
    required_references: &mut Vec<String>,
) {
    let Some(summary_body) = extract_compact_summary_body(message) else {
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
    for text in extract_message_text_fragments(message) {
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

pub fn build_compaction_preservation_hints(messages: &[Message]) -> CompactionPreservationHints {
    let mut active_request = Vec::new();
    let mut required_references = Vec::new();

    if let Some(previous_summary) = messages.iter().find(|message| message.is_compact_summary()) {
        collect_prior_summary_hints(
            previous_summary,
            &mut active_request,
            &mut required_references,
        );
    }

    let Some(active_request_start) = super::find_latest_external_request_block_start(messages)
    else {
        return CompactionPreservationHints {
            active_request,
            required_references,
        };
    };

    let request_block_end = messages[active_request_start..]
        .iter()
        .take_while(|message| message.is_external_request_message())
        .count()
        + active_request_start;

    for message in &messages[active_request_start..request_block_end] {
        for fragment in extract_message_text_fragments(message) {
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

    let reference_window_start =
        active_request_start.saturating_sub(REFERENCE_CONTEXT_WINDOW_MESSAGES);
    let mut raw_reference_candidates = Vec::new();
    for message in &messages[reference_window_start..request_block_end] {
        collect_reference_candidates_from_message(message, &mut raw_reference_candidates);
    }

    for candidate in raw_reference_candidates {
        push_unique_limited(
            &mut required_references,
            format_reference_candidate(&candidate),
            REQUIRED_REFERENCE_BULLET_LIMIT,
        );
    }

    CompactionPreservationHints {
        active_request,
        required_references,
    }
}
