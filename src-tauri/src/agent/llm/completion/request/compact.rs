use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use std::collections::HashMap;

const COMPACT_TOOL_SNAPSHOT_LIMIT: usize = 5;
const COMPACT_ARGUMENT_PREVIEW_LIMIT: usize = 96;
const COMPACT_RESULT_PREVIEW_LIMIT: usize = 140;

#[derive(Debug, Clone)]
struct CompactToolSnapshot {
    tool_name: String,
    argument_preview: String,
    status: &'static str,
    result_preview: String,
}

pub fn build_compact_summary_message(session_id: &str, text: String, created_at: i64) -> Message {
    Message::new_compact_summary_message(session_id, text, created_at)
}

pub fn build_compact_summary_message_for_messages(
    session_id: &str,
    summary: &str,
    compacted_messages: &[Message],
    created_at: i64,
) -> Message {
    build_compact_summary_message(
        session_id,
        build_compact_summary_text(summary, compacted_messages),
        created_at,
    )
}

pub fn build_compact_summary_text(summary: &str, compacted_messages: &[Message]) -> String {
    let mut text = format!("### Previous Conversation Summary\n\n{}", summary.trim());
    let recent_tool_snapshot = summarize_recent_tool_calls(compacted_messages);

    if !recent_tool_snapshot.is_empty() {
        text.push_str("\n\n### Recent Tool Call Snapshot (latest 5)\n");
        text.push_str(
            &recent_tool_snapshot
                .into_iter()
                .map(|entry| format!("- {}", entry))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }

    text
}

fn summarize_recent_tool_calls(compacted_messages: &[Message]) -> Vec<String> {
    let tool_results_by_id: HashMap<&str, &Message> = compacted_messages
        .iter()
        .filter(|message| message.role == "tool")
        .filter_map(|message| message.tool_call_id.as_deref().map(|id| (id, message)))
        .collect();

    let mut snapshots = Vec::new();

    for message in compacted_messages {
        if message.role != "assistant" {
            continue;
        }

        let Some(tool_calls) = &message.tool_calls else {
            continue;
        };

        for tool_call in tool_calls {
            let Some(result_message) = tool_results_by_id.get(tool_call.id.as_str()) else {
                continue;
            };

            snapshots.push(CompactToolSnapshot {
                tool_name: tool_call.function.name.clone(),
                argument_preview: build_tool_argument_preview(&tool_call.function.arguments),
                status: determine_tool_result_status(result_message),
                result_preview: extract_tool_result_preview(result_message),
            });
        }
    }

    snapshots
        .into_iter()
        .rev()
        .take(COMPACT_TOOL_SNAPSHOT_LIMIT)
        .map(|snapshot| {
            format!(
                "{}({}) -> {}: {}",
                snapshot.tool_name,
                snapshot.argument_preview,
                snapshot.status,
                snapshot.result_preview,
            )
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn build_tool_argument_preview(arguments: &str) -> String {
    let parsed = serde_json::from_str::<serde_json::Value>(arguments).ok();

    if let Some(serde_json::Value::Object(object)) = parsed {
        let mut preview_parts = Vec::new();
        let mut entries = object.iter().collect::<Vec<_>>();
        entries.sort_by_key(|(key, _)| *key);

        for (index, (key, value)) in entries.into_iter().enumerate() {
            if index >= 3 {
                preview_parts.push("...".to_string());
                break;
            }

            preview_parts.push(format!(
                "{}={}",
                key,
                truncate_for_compact_snapshot(&compact_json_value_preview(value), 32)
            ));
        }

        if preview_parts.is_empty() {
            "no-args".to_string()
        } else {
            truncate_for_compact_snapshot(&preview_parts.join(", "), COMPACT_ARGUMENT_PREVIEW_LIMIT)
        }
    } else if let Some(parsed_value) = parsed {
        truncate_for_compact_snapshot(&compact_json_value_preview(&parsed_value), 48)
    } else if arguments.trim().is_empty() {
        "no-args".to_string()
    } else {
        truncate_for_compact_snapshot(arguments.trim(), COMPACT_ARGUMENT_PREVIEW_LIMIT)
    }
}

fn compact_json_value_preview(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::Bool(flag) => flag.to_string(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Array(values) => {
            if values.is_empty() {
                "[]".to_string()
            } else {
                format!("[{} item(s)]", values.len())
            }
        }
        serde_json::Value::Object(values) => {
            if values.is_empty() {
                "{}".to_string()
            } else {
                format!("{{{} key(s)}}", values.len())
            }
        }
    }
}

fn determine_tool_result_status(message: &Message) -> &'static str {
    if message.error.is_some() {
        return "error";
    }

    for content in &message.content {
        if let MCPContent::Text {
            is_error: Some(true),
            ..
        } = content
        {
            return "error";
        }
    }

    "success"
}

fn extract_tool_result_preview(message: &Message) -> String {
    let mut text_parts = Vec::new();

    for content in &message.content {
        match content {
            MCPContent::Text { text, .. } => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    text_parts.push(trimmed.to_string());
                }
            }
            MCPContent::Resource { resource, .. } => {
                let mime_type = resource
                    .get("mimeType")
                    .and_then(|value| value.as_str())
                    .unwrap_or("resource");
                text_parts.push(format!("[resource:{}]", mime_type));
            }
            MCPContent::Image { .. } => text_parts.push("[image output]".to_string()),
            MCPContent::Audio { .. } => text_parts.push("[audio output]".to_string()),
            MCPContent::Thinking { .. } => {}
            MCPContent::ToolCall { name, .. } => {
                text_parts.push(format!("[tool call:{}]", name));
            }
        }
    }

    if text_parts.is_empty() {
        "completed with no textual result".to_string()
    } else {
        truncate_for_compact_snapshot(
            &text_parts.join(" | ").replace('\n', " "),
            COMPACT_RESULT_PREVIEW_LIMIT,
        )
    }
}

fn truncate_for_compact_snapshot(value: &str, max_len: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_len {
        return normalized;
    }

    let truncated = normalized
        .chars()
        .take(max_len.saturating_sub(1))
        .collect::<String>();
    format!("{}…", truncated)
}
