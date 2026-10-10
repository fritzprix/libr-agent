use crate::mcp::types::MCPContent;
use serde_json::Value;

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

pub fn latest_assistant_message_text(
    messages: &[Value],
    max_chars: Option<usize>,
) -> Option<(String, String)> {
    let mut first_assistant_id: Option<String> = None;

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

        if first_assistant_id.is_none() {
            first_assistant_id = Some(message_id.clone());
        }

        if let Some(text) = message.get("content").and_then(|v| v.as_str()) {
            let text = text.trim();
            if !text.is_empty() {
                let output = match max_chars {
                    Some(limit) if limit > 0 => truncate_text(text, limit),
                    _ => text.to_string(),
                };
                return Some((message_id, output));
            }
        }

        if let Some(content) = message.get("content").and_then(|v| v.as_array()) {
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
        }
    }

    first_assistant_id.map(|id| (id, "[assistant message has no text content]".to_string()))
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

/// Whether a tool-message content item is the `ui__reportResult` resource sibling
/// (paired with a text summary item in the same `content` array).
fn content_item_is_report_result_resource(item: &Value) -> bool {
    if item.get("type").and_then(|v| v.as_str()) != Some("resource") {
        return false;
    }

    let tool_name = item
        .get("serviceInfo")
        .and_then(|info| info.get("toolName"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if tool_name == "reportResult" {
        return true;
    }

    item.get("resource")
        .and_then(|resource| resource.get("uri"))
        .and_then(|v| v.as_str())
        .is_some_and(|uri| uri.starts_with("ui://result/"))
}

/// Extract the user-facing body from a `ui__reportResult` tool summary.
///
/// The tool wraps the deliverable as:
/// `...Result:\n{body}\n\nSTOP: Do not call any more tools...`
fn extract_report_result_body_from_summary(summary: &str) -> Option<String> {
    const RESULT_MARKER: &str = "Result:\n";
    const STOP_MARKER: &str = "\n\nSTOP:";

    let start = summary.find(RESULT_MARKER)? + RESULT_MARKER.len();
    let rest = &summary[start..];
    let body = match rest.find(STOP_MARKER) {
        Some(end) => &rest[..end],
        None => rest,
    };
    let body = body.trim();
    if body.is_empty() {
        None
    } else {
        Some(body.to_string())
    }
}

/// Newest `ui__reportResult` deliverable body and its index, if any.
///
/// Messages are newest-first: index `0` is the newest message.
fn latest_report_result_at(messages: &[Value]) -> Option<(usize, String)> {
    for (idx, message) in messages.iter().enumerate() {
        let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("");
        if role != "tool" {
            continue;
        }

        let Some(content) = message.get("content").and_then(|v| v.as_array()) else {
            continue;
        };

        // Identify via structuredContent OR resource sibling (legacy).
        let is_report_result = content.iter().any(content_item_is_report_result_resource)
            || message
                .get("metadata")
                .and_then(|m| m.get("structuredContent"))
                .is_some_and(|sc| {
                    sc.get("type").and_then(|v| v.as_str()) == Some("reportResult")
                        || (sc.get("criteria").is_some() && sc.get("result").is_some())
                });

        if !is_report_result {
            continue;
        }

        // Prefer structured `result` when persisted — survives text spillover /
        // marker drift better than parsing the STOP-wrapped summary.
        if let Some(body) = message
            .get("metadata")
            .and_then(|m| m.get("structuredContent"))
            .and_then(|sc| sc.get("result"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some((idx, body.to_string()));
        }

        for item in content {
            if item.get("type").and_then(|v| v.as_str()) != Some("text") {
                continue;
            }
            let Some(text) = item.get("text").and_then(|v| v.as_str()) else {
                continue;
            };
            if let Some(body) = extract_report_result_body_from_summary(text) {
                return Some((idx, body));
            }
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some((idx, trimmed.to_string()));
            }
        }
    }
    None
}

/// Newest `ui__reportResult` deliverable body, if any (messages are newest-first).
///
/// Prefer this over earlier assistant chatter when a child finished via
/// reportResult — unless a newer follow-up user turn produced assistant text
/// (see [`latest_session_output`]).
pub fn latest_report_result_body(messages: &[Value]) -> Option<String> {
    latest_report_result_at(messages).map(|(_, body)| body)
}

fn message_role_is(message: &Value, role: &str) -> bool {
    message.get("role").and_then(|v| v.as_str()) == Some(role)
}

/// Prefer `ui__reportResult` over earlier assistant narration, but let a
/// subsequent user→assistant follow-up override a stale report deliverable.
///
/// Messages are newest-first. A one-sentence confirmation after reportResult
/// (no new user turn) must not shadow the deliverable body. When a follow-up
/// user turn exists, only assistant text *after* that user message counts —
/// otherwise the pre-follow-up confirmation would win as soon as the parent
/// asks again, before the child has answered.
pub fn latest_session_output(messages: &[Value]) -> String {
    if let Some((report_idx, report_body)) = latest_report_result_at(messages) {
        // Newer than reportResult: indices 0..report_idx.
        let newer = &messages[..report_idx];
        // Oldest follow-up user in `newer` (nearest to the report). Valid
        // replies are strictly newer → lower indices → `&newer[..user_idx]`.
        if let Some(user_idx) = newer.iter().rposition(|m| message_role_is(m, "user")) {
            if let Some((_, follow_up_text)) =
                latest_assistant_message_text(&newer[..user_idx], None)
            {
                if follow_up_text != "[assistant message has no text content]" {
                    return follow_up_text;
                }
            }
        }
        return report_body;
    }

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

pub(super) fn format_message_summary(msg: &crate::models::chat::Message) -> String {
    let mut text_parts = Vec::new();
    for content in &msg.content {
        match content {
            MCPContent::Text { text, .. } => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    text_parts.push(trimmed.to_string());
                }
            }
            MCPContent::Thinking { thinking, .. } => {
                let trimmed = thinking.trim();
                if !trimmed.is_empty() {
                    text_parts.push(format!("[Thinking: {}]", trimmed));
                }
            }
            MCPContent::Image { .. } => {
                text_parts.push("[Image]".to_string());
            }
            MCPContent::Audio { .. } => {
                text_parts.push("[Audio]".to_string());
            }
            MCPContent::Resource { .. } => {
                text_parts.push("[Resource]".to_string());
            }
            _ => {}
        }
    }

    if text_parts.is_empty() {
        if let Some(tool_calls) = &msg.tool_calls {
            let names: Vec<String> = tool_calls
                .iter()
                .map(|tc| tc.function.name.clone())
                .collect();
            text_parts.push(format!("[Tool Call: {}]", names.join(", ")));
        }
    }

    let joined = text_parts.join(" ");
    if joined.chars().count() > 150 {
        let mut truncated: String = joined.chars().take(150).collect();
        truncated.push_str("...");
        truncated
    } else {
        joined
    }
}
