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

pub fn is_wait_complete_status(status: &str) -> bool {
    is_terminal_status(status) || status.eq_ignore_ascii_case("paused")
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn assistant_json(id: &str, text: &str) -> Value {
        json!({
            "id": id,
            "role": "assistant",
            "content": [{"type": "text", "text": text}]
        })
    }

    fn report_result_tool(id: &str, body: &str) -> Value {
        json!({
            "id": id,
            "role": "tool",
            "content": [
                {
                    "type": "text",
                    "text": format!(
                        "Final result reported (status=success).\nTitle: Result\nResult:\n{body}\n\nSTOP: Do not call any more tools."
                    )
                },
                {
                    "type": "resource",
                    "resource": {
                        "uri": format!("ui://result/{id}"),
                        "mimeType": "text/html",
                        "text": "<html></html>"
                    },
                    "serviceInfo": {
                        "serverName": "ui",
                        "toolName": "reportResult",
                        "backendType": "BuiltInRust"
                    }
                }
            ]
        })
    }

    #[test]
    fn latest_assistant_message_text_scans_past_empty_tool_call_turns() {
        // Message #2 (latest): tool call turn with no text content
        let latest_empty_asst = json!({
            "id": "asst-2",
            "role": "assistant",
            "content": [],
            "tool_calls": [{"id": "call-1", "function": {"name": "agent__checkSession"}}]
        });
        // Message #1 (earlier): contains the real final answer text
        let earlier_asst = assistant_json(
            "asst-1",
            "Consensus delegation review completed successfully.",
        );

        let messages = vec![latest_empty_asst, earlier_asst];

        let (msg_id, output) = latest_assistant_message_text(&messages, None).unwrap();
        assert_eq!(msg_id, "asst-1");
        assert_eq!(
            output,
            "Consensus delegation review completed successfully."
        );
        assert_eq!(
            latest_session_output(&messages),
            "Consensus delegation review completed successfully."
        );
    }

    #[test]
    fn latest_session_output_prefers_report_result_over_earlier_assistant_text() {
        // Mirrors production: child called ui__reportResult, then parent checkSession
        // used to return older assistant narration instead of the report body.
        let report_tool = json!({
            "id": "tool-report",
            "role": "tool",
            "content": [
                {
                    "type": "text",
                    "text": "Final result reported (status=success).\nTitle: Result\nResult:\n## Verdict: approve-with-caveats\n## Confidence: high\n\nSTOP: Do not call any more tools. The task outcome is already delivered. End your turn now with at most a one-sentence confirmation."
                },
                {
                    "type": "resource",
                    "resource": {
                        "uri": "ui://result/abc-123",
                        "mimeType": "text/html",
                        "text": "<html></html>"
                    },
                    "serviceInfo": {
                        "serverName": "ui",
                        "toolName": "reportResult",
                        "backendType": "BuiltInRust"
                    }
                }
            ]
        });
        let empty_asst = json!({
            "id": "asst-report-call",
            "role": "assistant",
            "content": [],
            "tool_calls": [{"id": "call-report", "function": {"name": "ui__reportResult"}}]
        });
        let earlier_asst = assistant_json(
            "asst-progress",
            "The persist_terminal test passes in isolation — continuing the review.",
        );

        let messages = vec![report_tool, empty_asst, earlier_asst];

        assert_eq!(
            latest_session_output(&messages),
            "## Verdict: approve-with-caveats\n## Confidence: high"
        );
    }

    #[test]
    fn latest_session_output_falls_back_to_report_result_text_without_markers() {
        // When the summary format drifts (no Result:/STOP markers), still prefer
        // the reportResult tool text over earlier assistant narration.
        let report_tool = json!({
            "id": "tool-report-plain",
            "role": "tool",
            "content": [
                {
                    "type": "text",
                    "text": "Deliverable without wrapper markers: approve-with-caveats"
                },
                {
                    "type": "resource",
                    "resource": {
                        "uri": "ui://result/plain-1",
                        "mimeType": "text/html",
                        "text": "<html></html>"
                    },
                    "serviceInfo": {
                        "serverName": "ui",
                        "toolName": "reportResult",
                        "backendType": "BuiltInRust"
                    }
                }
            ]
        });
        let earlier_asst = assistant_json("asst-old", "Earlier assistant chatter should lose.");

        let messages = vec![report_tool, earlier_asst];

        assert_eq!(
            latest_session_output(&messages),
            "Deliverable without wrapper markers: approve-with-caveats"
        );
    }

    #[test]
    fn latest_session_output_follow_up_assistant_overrides_stale_report_result() {
        // Parent messageToSession after reportResult: newer assistant text must win.
        // Newest-first: follow-up answer → user → confirmation → reportResult → call.
        let messages = vec![
            assistant_json(
                "asst-follow-up",
                "Here is the complete report:\n\n# Deep Research Report",
            ),
            json!({
                "id": "user-follow-up",
                "role": "user",
                "content": "Please output the full contents of the report."
            }),
            assistant_json("asst-confirm", "Done. The task outcome is delivered."),
            report_result_tool(
                "tool-report-stale",
                "Brief summary of outputs/rlcd_deep_dive.md",
            ),
            json!({
                "id": "asst-report-call",
                "role": "assistant",
                "content": [],
                "tool_calls": [{"id": "call-report", "function": {"name": "ui__reportResult"}}]
            }),
            assistant_json("asst-progress", "Still drafting the report…"),
        ];

        assert_eq!(
            latest_session_output(&messages),
            "Here is the complete report:\n\n# Deep Research Report"
        );
    }

    #[test]
    fn latest_session_output_confirmation_without_user_does_not_override_report_result() {
        // One-sentence confirmation after reportResult (no new user turn) must not
        // shadow the deliverable — that was the original prefer-reportResult fix.
        let messages = vec![
            assistant_json("asst-confirm", "Done. The task outcome is delivered."),
            report_result_tool(
                "tool-report",
                "## Verdict: approve-with-caveats\n## Confidence: high",
            ),
            json!({
                "id": "asst-report-call",
                "role": "assistant",
                "content": [],
                "tool_calls": [{"id": "call-report", "function": {"name": "ui__reportResult"}}]
            }),
        ];

        assert_eq!(
            latest_session_output(&messages),
            "## Verdict: approve-with-caveats\n## Confidence: high"
        );
    }

    #[test]
    fn latest_session_output_follow_up_without_assistant_text_keeps_report_result() {
        // Follow-up requested but child is still tool-calling — keep report until text arrives.
        let messages = vec![
            json!({
                "id": "asst-reading",
                "role": "assistant",
                "content": [],
                "tool_calls": [{"id": "call-read", "function": {"name": "workspace__readFile"}}]
            }),
            json!({
                "id": "user-follow-up",
                "role": "user",
                "content": "Please output the full contents."
            }),
            report_result_tool("tool-report", "Brief summary only"),
        ];

        assert_eq!(latest_session_output(&messages), "Brief summary only");
    }

    #[test]
    fn latest_session_output_follow_up_with_confirmation_still_keeps_report_result_until_answered()
    {
        // Realistic sequence: reportResult → short confirmation → parent follow-up
        // → child still tool-calling. Must not return the pre-follow-up confirmation.
        let messages = vec![
            json!({
                "id": "asst-reading",
                "role": "assistant",
                "content": [],
                "tool_calls": [{"id": "call-read", "function": {"name": "workspace__readFile"}}]
            }),
            json!({
                "id": "user-follow-up",
                "role": "user",
                "content": "Please output the full contents."
            }),
            assistant_json("asst-confirm", "Done. Result delivered."),
            report_result_tool("tool-report", "Brief summary only"),
        ];

        assert_eq!(latest_session_output(&messages), "Brief summary only");
    }

    #[test]
    fn latest_session_output_newer_report_result_wins_over_prior_follow_up() {
        // A second reportResult after a follow-up must become the new authoritative output.
        let messages = vec![
            report_result_tool("tool-report-2", "Updated deliverable after revision"),
            json!({
                "id": "asst-report-call-2",
                "role": "assistant",
                "content": [],
                "tool_calls": [{"id": "call-report-2", "function": {"name": "ui__reportResult"}}]
            }),
            assistant_json("asst-follow-up", "Here is the complete report (now stale)"),
            json!({
                "id": "user-follow-up",
                "role": "user",
                "content": "Please revise and report again."
            }),
            report_result_tool("tool-report-1", "Original brief summary"),
        ];

        assert_eq!(
            latest_session_output(&messages),
            "Updated deliverable after revision"
        );
    }
}
