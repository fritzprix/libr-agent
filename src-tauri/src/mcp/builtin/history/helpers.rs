use chrono::{DateTime, Utc};
use std::collections::HashMap;

use super::constants::PREVIEW_CHARS;
use super::types::{HistoryMessageListItem, HistorySessionItem};
use crate::mcp::types::MCPContent;
use crate::models::chat::Message;
use crate::repositories::{MessageRepository, SessionMetadata, SessionStatus};
use crate::utils::pagination::Page;

/// Normalize page number: clamp to minimum of 1.
pub fn normalize_page(page: u64) -> u64 {
    page.max(1)
}

/// Normalize page size: clamp to [1, MAX_PAGE_SIZE].
pub fn normalize_page_size(page_size: u64) -> u64 {
    use super::constants::MAX_PAGE_SIZE;
    page_size.clamp(1, MAX_PAGE_SIZE)
}

/// Normalize read-session order string.
pub fn normalize_read_session_order(order: Option<&str>) -> Result<&'static str, String> {
    use super::constants::DEFAULT_READ_SESSION_ORDER;
    let normalized = order
        .unwrap_or(DEFAULT_READ_SESSION_ORDER)
        .trim()
        .to_ascii_lowercase();
    match normalized.as_str() {
        "asc" => Ok("asc"),
        "desc" => Ok("desc"),
        _ => Err(format!(
            "Invalid order '{}'. Use 'asc' (oldest first) or 'desc' (newest first).",
            order.unwrap_or(DEFAULT_READ_SESSION_ORDER)
        )),
    }
}

/// Parse an optional status string into a SessionStatus.
pub fn parse_optional_status(status: Option<&str>) -> Result<Option<SessionStatus>, String> {
    status
        .map(|value| match value {
            "idle" => Ok(SessionStatus::Idle),
            "busy" => Ok(SessionStatus::Busy),
            "paused" => Ok(SessionStatus::Paused),
            "error" => Ok(SessionStatus::Error),
            _ => Err(format!(
                "Invalid status '{}'. Use idle, busy, paused, or error.",
                value
            )),
        })
        .transpose()
}

/// Parse an optional ISO-8601 timestamp string into a millisecond epoch.
pub fn parse_optional_timestamp(value: Option<&str>) -> Result<Option<i64>, String> {
    value.map(parse_timestamp).transpose()
}

/// Parse an ISO-8601 timestamp string into a millisecond epoch.
pub fn parse_timestamp(value: &str) -> Result<i64, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&Utc).timestamp_millis())
        .map_err(|_| format!("Invalid timestamp '{}'. Use ISO-8601.", value))
}

/// Build a map of session_id → message count for all sessions.
pub async fn build_message_counts(
    message_repo: &impl MessageRepository,
) -> Result<HashMap<String, u64>, String> {
    let counts = message_repo
        .count_by_session()
        .await
        .map_err(|e| e.to_string())?;
    Ok(counts.into_iter().collect())
}

/// Check whether a session matches the given agent/date/status filters.
pub fn session_matches_filters(
    session: &SessionMetadata,
    agent_id: &Option<String>,
    from: Option<i64>,
    to: Option<i64>,
    status_filter: Option<&SessionStatus>,
) -> bool {
    if let Some(expected_agent_id) = agent_id {
        if crate::agent::extract_assistant_id_from_session(session).as_deref()
            != Some(expected_agent_id.as_str())
        {
            return false;
        }
    }

    if let Some(status_filter) = status_filter {
        if &session.status != status_filter {
            return false;
        }
    }

    let reference_timestamp = session.last_message_at.unwrap_or(session.updated_at);
    timestamp_in_range(reference_timestamp, from, to)
}

/// Check whether a timestamp falls within an optional range.
pub fn timestamp_in_range(timestamp: i64, from: Option<i64>, to: Option<i64>) -> bool {
    if let Some(from) = from {
        if timestamp < from {
            return false;
        }
    }
    if let Some(to) = to {
        if timestamp > to {
            return false;
        }
    }
    true
}

/// Convert a SessionMetadata into a HistorySessionItem with message count.
pub fn to_history_session_item(
    session: SessionMetadata,
    message_counts: &HashMap<String, u64>,
) -> HistorySessionItem {
    let message_count = message_counts.get(&session.id).copied().unwrap_or(0);
    to_history_session_item_with_count(session, message_count)
}

/// Convert a SessionMetadata into a HistorySessionItem with an explicit count.
pub fn to_history_session_item_with_count(
    session: SessionMetadata,
    message_count: u64,
) -> HistorySessionItem {
    let agent_id = crate::agent::extract_assistant_id_from_session(&session);
    HistorySessionItem {
        session_id: session.id.clone(),
        name: session.name,
        status: session.status.as_str().to_string(),
        agent_id,
        parent_session_id: session.parent_session_id,
        lineage_id: session.lineage_id,
        created_at: session.created_at,
        updated_at: session.updated_at,
        last_message_at: session.last_message_at,
        message_count,
    }
}

/// Map a page of raw Message objects into a page of HistoryMessageListItem objects.
pub fn map_message_page(page: Page<Message>) -> Page<HistoryMessageListItem> {
    Page::new(
        page.items
            .into_iter()
            .map(to_history_message_list_item)
            .collect(),
        page.page,
        page.page_size,
        page.total_items,
    )
}

/// Convert a single Message into a HistoryMessageListItem.
pub fn to_history_message_list_item(message: Message) -> HistoryMessageListItem {
    let rendered = render_message_content(&message.content);
    HistoryMessageListItem {
        message_id: message.id,
        role: message.role,
        created_at: message.created_at,
        content_preview: bounded_snippet(rendered.clone(), PREVIEW_CHARS),
        content_length: rendered.chars().count(),
    }
}

/// Render a message's MCPContent array into a single display string.
pub fn render_message_content(content: &[MCPContent]) -> String {
    let parts: Vec<String> = content
        .iter()
        .map(|item| match item {
            MCPContent::Text { text, .. } => text.clone(),
            MCPContent::Thinking { thinking, .. } => {
                format!("[thinking]\n{}", thinking)
            }
            MCPContent::ToolCall {
                id,
                name,
                arguments,
            } => format!("[tool_call id={} name={}]\n{}", id, name, arguments),
            MCPContent::Image { mime_type, .. } => {
                format!("[image content: {}]", mime_type)
            }
            MCPContent::Audio { mime_type, .. } => {
                format!("[audio content: {}]", mime_type)
            }
            MCPContent::Resource {
                resource,
                service_info,
            } => format!(
                "[resource from {}::{}]\n{}",
                service_info.server_name,
                service_info.tool_name,
                serde_json::to_string_pretty(resource)
                    .unwrap_or_else(|_| "[unserializable resource]".to_string())
            ),
        })
        .filter(|part| !part.trim().is_empty())
        .collect();

    if parts.is_empty() {
        "[empty message content]".to_string()
    } else {
        parts.join("\n\n")
    }
}

/// Return a bounded preview snippet of text, truncating with ellipsis when needed.
pub fn bounded_snippet(text: String, max_chars: usize) -> String {
    let total_chars = text.chars().count();
    if total_chars <= max_chars {
        text
    } else {
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{truncated}…")
    }
}
