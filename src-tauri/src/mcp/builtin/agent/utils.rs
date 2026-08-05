use crate::agent::AgentSessionManager;
use crate::mcp::types::{MCPContent, MCPResult};
use crate::repositories::{MessageRepository, SessionMetadata};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

pub const CHECK_SESSION_RESULT_MESSAGE_LIMIT: u64 = 20;

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

pub fn build_agent_tool_data(
    tool_name: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    message: &str,
    response_status: &str,
    next_actions: Vec<Value>,
) -> serde_json::Map<String, Value> {
    let mut data = serde_json::Map::new();
    data.insert("toolName".to_string(), Value::String(tool_name.to_string()));
    data.insert(
        "resourceType".to_string(),
        Value::String(resource_type.to_string()),
    );
    data.insert("message".to_string(), Value::String(message.to_string()));
    data.insert(
        "responseStatus".to_string(),
        Value::String(response_status.to_string()),
    );

    if let Some(resource_id) = resource_id {
        data.insert(
            "resourceId".to_string(),
            Value::String(resource_id.to_string()),
        );
    }

    if !next_actions.is_empty() {
        data.insert("nextActions".to_string(), Value::Array(next_actions));
    }

    data
}

pub fn build_agent_session_tool_data(
    tool_name: &str,
    session_id: &str,
    message: &str,
    session_status: &str,
    response_status: &str,
    turn_count: usize,
    next_actions: Vec<Value>,
) -> serde_json::Map<String, Value> {
    let mut data = build_agent_tool_data(
        tool_name,
        "session",
        Some(session_id),
        message,
        response_status,
        next_actions,
    );
    data.insert(
        "sessionId".to_string(),
        Value::String(session_id.to_string()),
    );
    data.insert(
        "status".to_string(),
        Value::String(session_status.to_string()),
    );
    data.insert("turnCount".to_string(), json!(turn_count));
    data
}

async fn fetch_cached_session_messages_newest_first(
    session_id: &str,
    limit: usize,
) -> Option<Vec<Value>> {
    let sessions = crate::state::try_get_active_sessions()?;
    let active = sessions.read().await;
    let session = active.get(session_id)?;
    let cached_messages = session.messages.read().await;
    if cached_messages.is_empty() {
        return None;
    }

    let take = limit.min(cached_messages.len());
    let newest_first = cached_messages
        .iter()
        .rev()
        .take(take)
        .filter_map(|message| serde_json::to_value(message).ok())
        .collect::<Vec<_>>();

    Some(newest_first)
}

pub(crate) fn select_preferred_session_messages(
    db_messages: Vec<Value>,
    cached_messages: Option<Vec<Value>>,
) -> Vec<Value> {
    let Some(cached) = cached_messages else {
        return db_messages;
    };

    if let Some(db_latest) = db_messages.first() {
        if let Some(db_id) = db_latest.get("id").and_then(|id| id.as_str()) {
            let cache_contains_db_latest = cached
                .iter()
                .any(|m| m.get("id").and_then(|id| id.as_str()) == Some(db_id));
            if !cache_contains_db_latest {
                return db_messages;
            }
        }
    }

    let db_output = latest_session_output(&db_messages);
    let cached_output = latest_session_output(&cached);

    if session_output_is_missing(&cached_output) {
        return db_messages;
    }

    if session_output_is_missing(&db_output) {
        return cached;
    }

    if cached.len() > db_messages.len() {
        return cached;
    }

    if cached_output != db_output && cached.len() >= db_messages.len() {
        return cached;
    }

    db_messages
}

pub async fn fetch_session_messages_for_result(
    session_id: &str,
    limit: u64,
) -> Result<Vec<Value>, String> {
    let repo = crate::state::get_message_repository();
    let messages = repo
        .get_messages_by_session(session_id, limit)
        .await
        .map_err(|e| format!("Failed to fetch session messages: {}", e))?;

    let db_messages: Vec<Value> = messages
        .into_iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<Value>, serde_json::Error>>()
        .map_err(|e| format!("Failed to serialize session messages: {}", e))?;

    let cached = fetch_cached_session_messages_newest_first(session_id, limit as usize).await;

    Ok(select_preferred_session_messages(db_messages, cached))
}

pub fn extract_assistant_id_from_session_value(session: &Value) -> Option<String> {
    session
        .get("assistantId")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn session_metadata_to_value(session: &SessionMetadata) -> Result<Value, String> {
    let mut value =
        serde_json::to_value(session).map_err(|e| format!("Failed to serialize session: {}", e))?;

    if let Some(assistant_id) = extract_assistant_id_from_session_value(&value) {
        if let Some(object) = value.as_object_mut() {
            object.insert("assistantId".to_string(), Value::String(assistant_id));
        }
    }

    Ok(value)
}

pub async fn fetch_session_value(
    manager: &AgentSessionManager,
    session_id: &str,
) -> Result<Option<Value>, String> {
    manager
        .get_session(session_id)
        .await?
        .map(|session| session_metadata_to_value(&session))
        .transpose()
}

pub async fn count_session_turns(session_id: &str) -> usize {
    let repo = crate::state::get_message_repository();
    let messages = repo
        .get_messages_by_session(session_id, 1000)
        .await
        .unwrap_or_default();
    messages.iter().filter(|m| m.role == "assistant").count()
}

pub fn check_session_next_actions(session_id: &str) -> Vec<Value> {
    vec![
        json!({
            "toolName": "agent__checkSession",
            "reason": "Poll the session again for the latest status and turn count.",
            "args": {
                "sessionId": session_id,
                "wait": false
            }
        }),
        json!({
            "toolName": "agent__checkSession",
            "reason": "Block again later when you want to wait for a terminal result.",
            "args": {
                "sessionId": session_id,
                "wait": true
            }
        }),
    ]
}

pub fn read_required_string(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .ok_or_else(|| format!("Missing required parameter: {key}"))
}

pub async fn wait_until_session_terminal(
    manager: &AgentSessionManager,
    session_id: &str,
    timeout_seconds: u64,
    caller_session_id: Option<&str>,
) -> Result<(Value, u64), String> {
    const HEARTBEAT: Duration = Duration::from_secs(30);

    let bus = crate::state::get_session_bus();
    let child_notifier = bus.get_or_create(session_id);
    let caller_notifier = caller_session_id.map(|id| bus.get_or_create(id));
    let caller_cancel_pending: Option<Arc<std::sync::atomic::AtomicBool>> = match caller_session_id
    {
        Some(id) => crate::state::get_session_cancel_pending(id).await,
        None => None,
    };

    let started_at = Instant::now();
    let mut wake_count: u64 = 0;

    loop {
        if let Some(ref flag) = caller_cancel_pending {
            if flag.load(Ordering::Relaxed) {
                return Err(format!(
                    "agent__checkSession interrupted: calling session was cancelled while waiting for '{}'",
                    session_id
                ));
            }
        }

        let session = fetch_session_value(manager, session_id)
            .await?
            .ok_or_else(|| format!("Agent session '{}' not found", session_id))?;

        wake_count = wake_count.saturating_add(1);
        if is_wait_complete_status(&extract_session_status(&session)) {
            return Ok((session, wake_count));
        }

        let remaining = if timeout_seconds == 0 {
            None
        } else {
            let limit = Duration::from_secs(timeout_seconds.clamp(5, 86_400));
            let elapsed = started_at.elapsed();
            if elapsed >= limit {
                return Err(format!(
                    "agent__checkSession timed out after {}s for session {}",
                    timeout_seconds, session_id
                ));
            }
            Some(limit - elapsed)
        };

        let sleep_cap = remaining.map(|r| r.min(HEARTBEAT)).unwrap_or(HEARTBEAT);

        tokio::select! {
            _ = sleep(sleep_cap) => {}
            _ = child_notifier.notified() => {}
            _ = async {
                if let Some(ref notifier) = caller_notifier {
                    notifier.notified().await;
                } else {
                    futures::future::pending::<()>().await;
                }
            } => {}
        }
    }
}

fn format_message_summary(msg: &crate::models::chat::Message) -> String {
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

pub async fn handle_wait_timeout_result(
    wait_result: Result<(Value, u64), String>,
    manager: Option<&AgentSessionManager>,
    session_id: &str,
    timeout_seconds: u64,
    tool_name: &str,
    is_spawn: bool,
) -> Result<(Value, u64), Result<MCPResult, String>> {
    match wait_result {
        Ok(res) => Ok(res),
        Err(e) => {
            let (category, _) = crate::mcp::error_normalization::categorize_session_api_error(&e);
            if matches!(
                category,
                crate::mcp::error_normalization::ExternalMcpErrorCategory::Timeout
            ) {
                let (session_status, turn_count, latest_msgs_str, latest_msgs_json) = match manager
                {
                    Some(manager) => {
                        let session_status = match fetch_session_value(manager, session_id).await {
                            Ok(Some(session)) => extract_session_status(&session),
                            Ok(None) | Err(_) => "unknown".to_string(),
                        };
                        let turn_count = count_session_turns(session_id).await;

                        let repo = crate::state::get_message_repository();
                        let mut messages = repo
                            .get_messages_by_session(session_id, 5)
                            .await
                            .unwrap_or_default();
                        messages.reverse();

                        let mut msgs_str = String::new();
                        let mut msgs_json = Vec::new();

                        if !messages.is_empty() {
                            msgs_str.push_str("\n\nLatest workflow messages:\n");
                            for msg in &messages {
                                let summary = format_message_summary(msg);
                                msgs_str.push_str(&format!("  - [{}]: {}\n", msg.role, summary));
                                msgs_json.push(json!({
                                    "role": msg.role,
                                    "summary": summary,
                                    "createdAt": msg.created_at,
                                }));
                            }
                        }

                        (session_status, turn_count, msgs_str, msgs_json)
                    }
                    None => ("unknown".to_string(), 0, String::new(), Vec::new()),
                };

                let text = if is_spawn {
                    format!(
                        "Child session created (ID: {}) but waiting for completion timed out after {}s.\n\nThe agent is likely still working. Use agent__checkSession(sessionId=\"{}\", wait=true) later to fetch the final result.{}\n\nCurrent status: {}",
                        session_id, timeout_seconds, session_id, latest_msgs_str, session_status
                    )
                } else {
                    format!(
                        "Waiting for session {} timed out after {}s. The agent is likely still working.\n\nYou can call agent__checkSession(sessionId=\"{}\", wait=true) again to continue waiting, or use agent__listAgents(type=\"sessions\") to confirm it is still active.{}\n\nCurrent status: {}",
                        session_id, timeout_seconds, session_id, latest_msgs_str, session_status
                    )
                };

                let mut data = build_agent_session_tool_data(
                    tool_name,
                    session_id,
                    &text,
                    &session_status,
                    "timeout",
                    turn_count,
                    check_session_next_actions(session_id),
                );
                data.insert("timeout".to_string(), json!(true));
                data.insert("timeoutSeconds".to_string(), json!(timeout_seconds));
                data.insert(
                    "errorCategory".to_string(),
                    Value::String("timeout".to_string()),
                );
                data.insert("error".to_string(), Value::String(e));
                data.insert("latestMessages".to_string(), json!(latest_msgs_json));

                if is_spawn {
                    data.insert("id".to_string(), Value::String(session_id.to_string()));
                }

                Err(Ok(MCPResult {
                    content: Some(vec![MCPContent::Text { text }]),
                    structured_content: Some(Value::Object(data)),
                    is_error: Some(false),
                }))
            } else {
                Err(Err(e))
            }
        }
    }
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

    #[test]
    fn select_preferred_prefers_cache_when_db_lags_behind_terminal_assistant() {
        let db_messages = vec![assistant_json("asst-1", "older answer")];
        let cached_messages = vec![
            assistant_json("asst-2", "final answer"),
            assistant_json("asst-1", "older answer"),
        ];

        let selected = select_preferred_session_messages(db_messages, Some(cached_messages));

        assert_eq!(latest_session_output(&selected), "final answer");
    }

    #[test]
    fn select_preferred_keeps_db_when_cache_is_stale() {
        let db_messages = vec![assistant_json("asst-2", "authoritative answer")];
        let cached_messages = vec![assistant_json("asst-1", "stale cache")];

        let selected = select_preferred_session_messages(db_messages, Some(cached_messages));

        assert_eq!(latest_session_output(&selected), "authoritative answer");
    }
}
