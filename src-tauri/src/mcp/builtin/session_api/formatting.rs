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

/// Returns true if the last tool-role message in `messages` contains a
/// content item with `"type": "resource"` — indicating an intentional pause
/// waiting for user interaction (e.g. a UI resource prompt).
pub fn last_message_is_ui_resource(messages: &[Value]) -> bool {
    messages
        .iter()
        .rev()
        .find(|m| m.get("role").and_then(|v| v.as_str()) == Some("tool"))
        .map(|m| {
            m.get("content")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .any(|item| item.get("type").and_then(|v| v.as_str()) == Some("resource"))
                })
                .unwrap_or(false)
        })
        .unwrap_or(false)
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

        for item in content.iter().rev() {
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

pub fn latest_tool_message_text(messages: &[Value]) -> Option<String> {
    for message in messages {
        let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("");

        if role != "tool" {
            continue;
        }

        if let Some(content) = message.get("content").and_then(|v| v.as_array()) {
            for item in content {
                let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if item_type == "text" {
                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                        let text = text.trim();
                        if !text.is_empty() {
                            return Some(text.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn latest_session_output(messages: &[Value]) -> String {
    let (_, mut assistant_text) = latest_assistant_message_text(messages, None)
        .unwrap_or(("none".to_string(), "No final answer yet.".to_string()));

    if assistant_text == "[assistant message has no text content]" {
        if let Some(tool_text) = latest_tool_message_text(messages) {
            assistant_text = format!("[Tool Response Fallback]\n{}", tool_text);
        }
    }

    assistant_text
}

pub fn session_output_is_missing(output: &str) -> bool {
    matches!(
        output,
        "No final answer yet." | "[assistant message has no text content]"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -----------------------------------------------------------------------
    // last_message_is_ui_resource
    // -----------------------------------------------------------------------

    /// Basic happy path: last tool message contains a "resource" content item.
    /// This identifies an *intentional* pause waiting for UI interaction, so
    /// awaitAgent must NOT inject a recovery kick.
    #[test]
    fn ui_resource_returns_true_when_last_tool_has_resource() {
        let messages = vec![
            json!({ "role": "user",      "content": [{"type": "text", "text": "hi"}] }),
            json!({ "role": "assistant", "content": [{"type": "tool_call", "name": "foo"}] }),
            json!({ "role": "tool",      "content": [{"type": "resource", "uri": "ui://form/abc"}] }),
        ];
        assert!(last_message_is_ui_resource(&messages));
    }

    /// Tool message exists but has only text content → NOT a UI resource pause.
    #[test]
    fn ui_resource_returns_false_when_last_tool_has_only_text() {
        let messages =
            vec![json!({ "role": "tool", "content": [{"type": "text", "text": "done"}] })];
        assert!(!last_message_is_ui_resource(&messages));
    }

    /// No tool messages at all → false.
    #[test]
    fn ui_resource_returns_false_with_no_tool_messages() {
        let messages = vec![
            json!({ "role": "user",      "content": [{"type": "text", "text": "hello"}] }),
            json!({ "role": "assistant", "content": [{"type": "text", "text": "hey"}] }),
        ];
        assert!(!last_message_is_ui_resource(&messages));
    }

    /// Empty message list → false.
    #[test]
    fn ui_resource_returns_false_for_empty_list() {
        assert!(!last_message_is_ui_resource(&[]));
    }

    /// Only the *last* tool message matters.
    /// Earlier tool messages with resource content must not cause a false positive.
    #[test]
    fn ui_resource_only_last_tool_message_matters() {
        let messages = vec![
            // Earlier tool: has resource (should be ignored)
            json!({ "role": "tool",      "content": [{"type": "resource", "uri": "ui://old"}] }),
            json!({ "role": "assistant", "content": [{"type": "text", "text": "processing"}] }),
            // Last tool: plain text result (crash scenario / normal completion)
            json!({ "role": "tool",      "content": [{"type": "text", "text": "result"}] }),
        ];
        assert!(!last_message_is_ui_resource(&messages));
    }

    /// A tool message with *mixed* content (resource + text) should still return true.
    #[test]
    fn ui_resource_returns_true_for_mixed_content_with_resource() {
        let messages = vec![json!({
            "role": "tool",
            "content": [
                {"type": "text", "text": "Here is your form"},
                {"type": "resource", "uri": "ui://form/xyz"}
            ]
        })];
        assert!(last_message_is_ui_resource(&messages));
    }

    /// Non-tool roles with "resource" type must NOT trigger the check.
    #[test]
    fn ui_resource_ignores_resource_in_non_tool_roles() {
        let messages = vec![
            json!({ "role": "assistant", "content": [{"type": "resource", "uri": "ui://foo"}] }),
        ];
        assert!(!last_message_is_ui_resource(&messages));
    }

    // -----------------------------------------------------------------------
    // is_terminal_status – sanity check for crash-recovery poll-loop exit
    // -----------------------------------------------------------------------

    #[test]
    fn terminal_status_idle_is_terminal() {
        assert!(is_terminal_status("idle"));
        assert!(is_terminal_status("Idle")); // case-insensitive
    }

    #[test]
    fn terminal_status_busy_is_not_terminal() {
        assert!(!is_terminal_status("busy"));
    }

    #[test]
    fn terminal_status_paused_is_not_terminal() {
        assert!(!is_terminal_status("paused"));
    }

    #[test]
    fn terminal_status_error_is_terminal() {
        assert!(is_terminal_status("error"));
        assert!(is_terminal_status("failed"));
    }

    #[test]
    fn latest_assistant_message_text_prefers_last_text_block_in_content() {
        let messages = vec![json!({
            "role": "assistant",
            "id": "assistant-final",
            "content": [
                {"type": "text", "text": "Working on it..."},
                {"type": "thinking", "thinking": "internal reasoning"},
                {"type": "text", "text": "Final answer for the delegated task."}
            ]
        })];

        let (message_id, text) = latest_assistant_message_text(&messages, None)
            .expect("assistant text should be found");

        assert_eq!(message_id, "assistant-final");
        assert_eq!(text, "Final answer for the delegated task.");
    }

    #[test]
    fn latest_tool_message_text_skips_leading_assistant_without_text() {
        let messages = vec![
            json!({
                "role": "assistant",
                "content": [{"type": "tool_call", "name": "workspace__readFile"}]
            }),
            json!({
                "role": "tool",
                "content": [{"type": "text", "text": "Structured tool output summary"}]
            }),
        ];

        assert_eq!(
            latest_tool_message_text(&messages).as_deref(),
            Some("Structured tool output summary")
        );
    }
}
