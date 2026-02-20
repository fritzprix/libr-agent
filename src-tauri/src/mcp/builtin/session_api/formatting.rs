use serde_json::Value;
use std::collections::HashMap;

use super::types::MessageSummaryOptions;

pub fn truncate_text(input: &str, max_chars: usize) -> String {
    let normalized = input.replace('\n', " ").trim().to_string();
    if normalized.chars().count() <= max_chars {
        return normalized;
    }

    let mut truncated = String::new();
    for ch in normalized.chars().take(max_chars) {
        truncated.push(ch);
    }
    truncated.push_str("...");
    truncated
}

pub fn extract_assistant_description(config: &Value) -> String {
    if let Some(description) = config.get("description").and_then(|v| v.as_str()) {
        let cleaned = description.trim();
        if !cleaned.is_empty() {
            return truncate_text(cleaned, 140);
        }
    }

    if let Some(system_prompt) = config.get("systemPrompt").and_then(|v| v.as_str()) {
        let first_meaningful_line = system_prompt
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("");

        if !first_meaningful_line.is_empty() {
            return truncate_text(first_meaningful_line, 140);
        }
    }

    "No description".to_string()
}

pub fn extract_session_status(session: &Value) -> String {
    session
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string()
}

pub fn is_terminal_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "idle" | "terminated" | "failed" | "error"
    )
}

pub fn message_preview_text(message: &Value, options: MessageSummaryOptions) -> Option<String> {
    let role = message
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let message_id = message
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let content = message.get("content").and_then(|v| v.as_array())?;
    let text_snippet_limit = if options.include_raw_preview {
        260
    } else {
        120
    };
    let line_snippet_limit = if options.include_raw_preview {
        300
    } else {
        160
    };

    let mut snippets: Vec<String> = Vec::new();
    for item in content {
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match item_type {
            "text" => {
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    let text = text.trim();
                    if !text.is_empty() {
                        snippets.push(truncate_text(text, text_snippet_limit));
                    }
                }
            }
            "tool_call" => {
                let tool_name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                snippets.push(format!("[tool_call:{}]", tool_name));
            }
            _ => {}
        }
    }

    let preview = if snippets.is_empty() {
        "[non-text content]".to_string()
    } else {
        truncate_text(&snippets.join(" | "), line_snippet_limit)
    };

    if options.summary_only {
        Some(format!("• [{}] {}", role, preview))
    } else {
        Some(format!("• [{}] {} :: {}", role, message_id, preview))
    }
}

pub fn build_messages_summary(
    messages: &[Value],
    session_id: &str,
    options: MessageSummaryOptions,
) -> String {
    let message_count = messages.len();
    if message_count == 0 {
        return format!("Fetched 0 messages for session {}", session_id);
    }

    let previews = messages
        .iter()
        .take(options.preview_limit)
        .filter_map(|message| message_preview_text(message, options))
        .collect::<Vec<_>>();

    if previews.is_empty() {
        return format!(
            "Fetched {} messages for session {}",
            message_count, session_id
        );
    }

    let mode_hint = if options.summary_only {
        "summary-only"
    } else {
        "expanded"
    };

    format!(
        "Fetched {} messages for session {} (mode: {})\n\nRecent message previews (latest first):\n{}",
        message_count,
        session_id,
        mode_hint,
        previews.join("\n")
    )
}

pub fn build_swarm_snapshot_text(
    root_session_id: &str,
    rows: &[(String, String, String, usize, Option<String>)],
    truncated: bool,
    max_nodes: usize,
) -> String {
    if rows.is_empty() {
        return format!(
            "Swarm board: no active sub-agents under current command session {}.\nNext step: use spawnAgent to deploy a worker.",
            root_session_id
        );
    }

    let direct_count = rows
        .iter()
        .filter(|(_, _, _, depth, _)| *depth == 1)
        .count();
    let total_count = rows.len();

    let mut status_counts: HashMap<String, usize> = HashMap::new();
    for (_, _, status, _, _) in rows {
        *status_counts.entry(status.clone()).or_insert(0) += 1;
    }

    let mut status_parts = status_counts
        .iter()
        .map(|(status, count)| format!("{}:{}", status, count))
        .collect::<Vec<_>>();
    status_parts.sort();

    let mut text = format!(
        "Swarm command board (commander session: {})\n- Direct units: {}\n- Total descendants: {}\n- Status breakdown: {}\n\nUnit roster:\n",
        root_session_id,
        direct_count,
        total_count,
        status_parts.join(", ")
    );

    for (session_id, name, status, depth, preview) in rows {
        let indent = "  ".repeat(depth.saturating_sub(1));
        let mut line = format!(
            "- {}{} (ID: {}) status={} depth={}\n",
            indent, name, session_id, status, depth
        );

        if let Some(summary) = preview {
            line.push_str(&format!("  {}latest assistant: {}\n", indent, summary));
        }

        text.push_str(&line);
    }

    if truncated {
        text.push_str(&format!(
            "\nRoster truncated at {} units. Use specific session IDs with getAgentLog/getAgentStatus for deeper checks.",
            max_nodes
        ));
    }

    text
}

pub fn latest_assistant_message_text(
    messages: &[Value],
    max_chars: Option<usize>,
) -> Option<(String, String)> {
    for message in messages {
        let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("");
        if role != "assistant" {
            continue;
        }

        let message_id = message
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let content = match message.get("content").and_then(|v| v.as_array()) {
            Some(content) => content,
            None => continue,
        };

        for item in content {
            let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if item_type != "text" {
                continue;
            }

            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                let text = text.trim();
                if !text.is_empty() {
                    let output = match max_chars {
                        Some(limit) if limit > 0 => truncate_text(text, limit),
                        _ => text.to_string(),
                    };
                    return Some((message_id, output));
                }
            }
        }

        return Some((
            message_id,
            "[assistant message has no text content]".to_string(),
        ));
    }

    None
}
