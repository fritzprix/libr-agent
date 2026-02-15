use serde_json::Value;
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
