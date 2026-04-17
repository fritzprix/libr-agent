use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use crate::mcp::builtin::error_guidance::{operation_failed_error, SuccessHint, ToolGroup};
use crate::mcp::builtin::history::types::{
    HistoryMessageListItem, HistoryMessageReadResponse, HistorySearchMatch, HistorySessionItem,
    HistorySessionReadResponse,
};
use crate::mcp::builtin::history::HistoryServer;
use crate::mcp::types::{MCPContent, MCPResult};
use crate::models::chat::Message;
use crate::repositories::{MessageRepository, SessionMetadata, SessionRepository, SessionStatus};
use crate::services::message_service::MessageService;
use crate::state::{get_message_repository, get_session_repository};
use crate::utils::pagination::{paginate_in_memory, Page};

const DEFAULT_LIST_PAGE: u64 = 1;
const DEFAULT_LIST_PAGE_SIZE: u64 = 20;
const DEFAULT_MESSAGE_PAGE_SIZE: u64 = 50;
const DEFAULT_SEARCH_PAGE_SIZE: u64 = 20;
const MAX_PAGE_SIZE: u64 = 100;
const MAX_MESSAGE_CHARS: usize = 3000;
const PREVIEW_CHARS: usize = 240;
const SEARCH_SCAN_LIMIT: u64 = 1000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListArgs {
    agent_id: Option<String>,
    from: Option<String>,
    to: Option<String>,
    status: Option<String>,
    page: Option<u64>,
    page_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadSessionArgs {
    session_id: String,
    page: Option<u64>,
    page_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadMessageArgs {
    message_id: String,
    offset_chars: Option<usize>,
    max_chars: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchArgs {
    query: String,
    agent_id: Option<String>,
    session_id: Option<String>,
    from: Option<String>,
    to: Option<String>,
    roles: Option<Vec<String>>,
    page: Option<u64>,
    page_size: Option<u64>,
}

pub async fn list_sessions(_server: &HistoryServer, args: Value) -> Result<MCPResult, String> {
    let args: ListArgs =
        serde_json::from_value(args).map_err(|e| format!("Invalid list arguments: {e}"))?;
    let page = normalize_page(args.page.unwrap_or(DEFAULT_LIST_PAGE));
    let page_size = normalize_page_size(args.page_size.unwrap_or(DEFAULT_LIST_PAGE_SIZE));
    let from = parse_optional_timestamp(args.from.as_deref())?;
    let to = parse_optional_timestamp(args.to.as_deref())?;
    let status_filter = parse_optional_status(args.status.as_deref())?;

    let session_repo = get_session_repository();
    let message_repo = get_message_repository();
    let sessions = session_repo
        .get_all_sessions()
        .await
        .map_err(|e| e.to_string())?;
    let message_counts = build_message_counts(message_repo).await?;

    let mut filtered: Vec<HistorySessionItem> = sessions
        .into_iter()
        .filter(|session| {
            session_matches_filters(session, &args.agent_id, from, to, status_filter.as_ref())
        })
        .map(|session| to_history_session_item(session, &message_counts))
        .collect();
    filtered.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.session_id.cmp(&right.session_id))
    });

    let paged = paginate_in_memory(filtered, page, page_size);
    let text = render_list_text(&paged);

    Ok(SuccessHint::new(
        text,
        vec!["Use readSession(sessionId=\"...\") to inspect messages".to_string()],
    )
    .to_mcp_result_with_data(Some(json!({
        "sessions": paged.items,
        "page": paged.page,
        "pageSize": paged.page_size,
        "totalItems": paged.total_items,
        "totalPages": paged.total_pages,
        "hasNextPage": paged.has_next_page,
        "hasPreviousPage": paged.has_previous_page
    }))))
}

pub async fn read_session(_server: &HistoryServer, args: Value) -> Result<MCPResult, String> {
    let args: ReadSessionArgs =
        serde_json::from_value(args).map_err(|e| format!("Invalid readSession arguments: {e}"))?;
    let page = normalize_page(args.page.unwrap_or(DEFAULT_LIST_PAGE));
    let page_size = normalize_page_size(args.page_size.unwrap_or(DEFAULT_MESSAGE_PAGE_SIZE));

    let session_repo = get_session_repository();
    let message_repo = get_message_repository();

    let session = match session_repo
        .get_session(&args.session_id)
        .await
        .map_err(|e| e.to_string())?
    {
        Some(session) => session,
        None => {
            return Ok(operation_failed_error(
                "Read Session",
                &format!("Session '{}' not found", args.session_id),
                vec![
                    "Use list() to find a valid session ID".to_string(),
                    "Retry readSession with a copied sessionId from list()".to_string(),
                ],
                ToolGroup::Agent,
            ));
        }
    };

    let message_page = message_repo
        .get_page(&args.session_id, page, page_size)
        .await
        .map_err(|e| e.to_string())?;
    let total_messages = message_page.total_items;
    let messages = map_message_page(message_page);

    let response = HistorySessionReadResponse {
        session: to_history_session_item_with_count(session, total_messages),
        messages,
    };

    let text = render_read_session_text(&response);

    Ok(SuccessHint::new(
        text,
        vec!["Use readMessage(messageId=\"...\") to page through a large message body".to_string()],
    )
    .to_mcp_result_with_data(Some(json!(response))))
}

pub async fn read_message(_server: &HistoryServer, args: Value) -> Result<MCPResult, String> {
    let args: ReadMessageArgs =
        serde_json::from_value(args).map_err(|e| format!("Invalid readMessage arguments: {e}"))?;
    let offset_chars = args.offset_chars.unwrap_or(0);
    let max_chars = args
        .max_chars
        .unwrap_or(MAX_MESSAGE_CHARS)
        .min(MAX_MESSAGE_CHARS);

    let message_repo = get_message_repository();
    let message = match message_repo
        .get_by_id(&args.message_id)
        .await
        .map_err(|e| e.to_string())?
    {
        Some(message) => message,
        None => {
            return Ok(operation_failed_error(
                "Read Message",
                &format!("Message '{}' not found", args.message_id),
                vec![
                    "Use readSession(sessionId=\"...\") to list message IDs".to_string(),
                    "Retry readMessage with a copied messageId from readSession()".to_string(),
                ],
                ToolGroup::Agent,
            ));
        }
    };

    let rendered = render_message_content(&message.content);
    let total_chars = rendered.chars().count();
    let (content_chunk, chunk_length, has_more, next_offset) =
        slice_text_chunk(&rendered, total_chars, offset_chars, max_chars);

    let response = HistoryMessageReadResponse {
        message_id: message.id.clone(),
        session_id: message.session_id.clone(),
        role: message.role.clone(),
        created_at: message.created_at,
        total_chars,
        chunk_offset: offset_chars.min(total_chars),
        chunk_length,
        has_more,
        next_offset,
        content_chunk: content_chunk.clone(),
    };

    let text = render_read_message_text(&response);

    Ok(SuccessHint::new(
        text,
        next_offset
            .map(|offset| {
                vec![format!(
                    "Use readMessage(messageId=\"{}\", offsetChars={offset}) for the next chunk",
                    message.id
                )]
            })
            .unwrap_or_default(),
    )
    .to_mcp_result_with_data(Some(json!({
        "message": response,
        "rawMessage": message
    }))))
}

pub async fn search_history(
    _server: &HistoryServer,
    args: Value,
    caller_session_id: &str,
) -> Result<MCPResult, String> {
    let args: SearchArgs =
        serde_json::from_value(args).map_err(|e| format!("Invalid search arguments: {e}"))?;
    let page = normalize_page(args.page.unwrap_or(DEFAULT_LIST_PAGE));
    let page_size = normalize_page_size(args.page_size.unwrap_or(DEFAULT_SEARCH_PAGE_SIZE));
    let from = parse_optional_timestamp(args.from.as_deref())?;
    let to = parse_optional_timestamp(args.to.as_deref())?;
    let role_filter = args
        .roles
        .map(|roles| roles.into_iter().collect::<HashSet<_>>());
    if let Some(session_id) = args.session_id.as_deref() {
        let session_repo = get_session_repository();
        let session_exists = session_repo
            .get_session(session_id)
            .await
            .map_err(|e| e.to_string())?
            .is_some();
        if !session_exists {
            return Ok(operation_failed_error(
                "Search History",
                &format!("Session '{}' not found", session_id),
                vec![
                    "Use list() to find a valid session ID".to_string(),
                    "Retry search() with a copied sessionId from list()".to_string(),
                ],
                ToolGroup::Agent,
            ));
        }
    }

    let allowed_session_ids = resolve_allowed_session_ids(
        args.session_id.as_deref(),
        args.agent_id.as_deref(),
        from,
        to,
    )
    .await?;

    if let Some(session_id) = args.session_id.as_deref() {
        if !allowed_session_ids.contains(session_id) {
            return Ok(operation_failed_error(
                "Search History",
                &format!(
                    "Session '{}' did not match the provided filters",
                    session_id
                ),
                vec![
                    "Use list() to confirm the session ID".to_string(),
                    "Relax agentId/from/to filters and try search() again".to_string(),
                ],
                ToolGroup::Agent,
            ));
        }
    }

    let scan_page_size = page
        .saturating_mul(page_size)
        .saturating_mul(10)
        .clamp(page_size, SEARCH_SCAN_LIMIT);

    let raw_page = MessageService::search_messages(
        args.query.clone(),
        args.session_id.clone(),
        1,
        scan_page_size,
    )
    .await?;

    let message_repo = get_message_repository();
    let messages = message_repo
        .get_by_ids(
            raw_page
                .items
                .iter()
                .map(|item| item.message_id.clone())
                .collect(),
        )
        .await
        .map_err(|e| e.to_string())?;
    let message_map: HashMap<String, Message> = messages
        .into_iter()
        .map(|message| (message.id.clone(), message))
        .collect();

    let filtered_matches: Vec<HistorySearchMatch> = raw_page
        .items
        .into_iter()
        .filter(|item| allowed_session_ids.contains(&item.session_id))
        .filter_map(|item| {
            let message = message_map.get(&item.message_id)?;
            if let Some(role_filter) = &role_filter {
                if !role_filter.contains(&message.role) {
                    return None;
                }
            }
            if !timestamp_in_range(message.created_at, from, to) {
                return None;
            }
            let rendered_content = render_message_content(&message.content);
            let content_length = rendered_content.chars().count();
            let snippet_source = item.snippet.unwrap_or_else(|| rendered_content.clone());

            Some(HistorySearchMatch {
                session_id: item.session_id,
                message_id: item.message_id,
                role: message.role.clone(),
                created_at: item.created_at,
                score: item.score,
                snippet: bounded_snippet(snippet_source, PREVIEW_CHARS),
                content_length,
            })
        })
        .collect();

    let paged = paginate_in_memory(filtered_matches, page, page_size);
    let text = render_search_text(&paged, caller_session_id);

    Ok(SuccessHint::new(
        text,
        vec![
            "Use readSession(sessionId=\"...\") to inspect the surrounding conversation"
                .to_string(),
            "Use readMessage(messageId=\"...\") to expand a specific hit".to_string(),
        ],
    )
    .to_mcp_result_with_data(Some(json!({
        "matches": paged.items,
        "page": paged.page,
        "pageSize": paged.page_size,
        "totalItems": paged.total_items,
        "totalPages": paged.total_pages,
        "hasNextPage": paged.has_next_page,
        "hasPreviousPage": paged.has_previous_page
    }))))
}

fn normalize_page(page: u64) -> u64 {
    page.max(1)
}

fn normalize_page_size(page_size: u64) -> u64 {
    page_size.clamp(1, MAX_PAGE_SIZE)
}

fn parse_optional_status(status: Option<&str>) -> Result<Option<SessionStatus>, String> {
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

fn parse_optional_timestamp(value: Option<&str>) -> Result<Option<i64>, String> {
    value.map(parse_timestamp).transpose()
}

fn parse_timestamp(value: &str) -> Result<i64, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&Utc).timestamp_millis())
        .map_err(|_| format!("Invalid timestamp '{}'. Use ISO-8601.", value))
}

async fn build_message_counts(
    message_repo: &impl MessageRepository,
) -> Result<HashMap<String, u64>, String> {
    let counts = message_repo
        .count_by_session()
        .await
        .map_err(|e| e.to_string())?;
    Ok(counts.into_iter().collect())
}

fn session_matches_filters(
    session: &SessionMetadata,
    agent_id: &Option<String>,
    from: Option<i64>,
    to: Option<i64>,
    status_filter: Option<&SessionStatus>,
) -> bool {
    if let Some(expected_agent_id) = agent_id {
        if extract_agent_id(session.agent_config.as_deref()).as_deref()
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

fn timestamp_in_range(timestamp: i64, from: Option<i64>, to: Option<i64>) -> bool {
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

fn extract_agent_id(agent_config: Option<&str>) -> Option<String> {
    let parsed: Value = serde_json::from_str(agent_config?).ok()?;
    parsed
        .get("assistantId")
        .and_then(Value::as_str)
        .or_else(|| parsed.get("assistant_id").and_then(Value::as_str))
        .or_else(|| parsed.get("id").and_then(Value::as_str))
        .map(str::to_string)
}

fn to_history_session_item(
    session: SessionMetadata,
    message_counts: &HashMap<String, u64>,
) -> HistorySessionItem {
    let message_count = message_counts.get(&session.id).copied().unwrap_or(0);
    to_history_session_item_with_count(session, message_count)
}

fn to_history_session_item_with_count(
    session: SessionMetadata,
    message_count: u64,
) -> HistorySessionItem {
    HistorySessionItem {
        session_id: session.id.clone(),
        name: session.name,
        status: session.status.as_str().to_string(),
        agent_id: extract_agent_id(session.agent_config.as_deref()),
        parent_session_id: session.parent_session_id,
        lineage_id: session.lineage_id,
        created_at: session.created_at,
        updated_at: session.updated_at,
        last_message_at: session.last_message_at,
        message_count,
    }
}

fn map_message_page(page: Page<Message>) -> Page<HistoryMessageListItem> {
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

fn to_history_message_list_item(message: Message) -> HistoryMessageListItem {
    let rendered = render_message_content(&message.content);
    HistoryMessageListItem {
        message_id: message.id,
        role: message.role,
        created_at: message.created_at,
        content_preview: bounded_snippet(rendered.clone(), PREVIEW_CHARS),
        content_length: rendered.chars().count(),
    }
}

fn render_message_content(content: &[MCPContent]) -> String {
    let parts: Vec<String> = content
        .iter()
        .map(|item| match item {
            MCPContent::Text { text, .. } => text.clone(),
            MCPContent::Thinking { thinking, .. } => format!("[thinking]\n{}", thinking),
            MCPContent::ToolCall {
                id,
                name,
                arguments,
            } => format!("[tool_call id={} name={}]\n{}", id, name, arguments),
            MCPContent::Image { mime_type, .. } => format!("[image content: {}]", mime_type),
            MCPContent::Audio { mime_type, .. } => format!("[audio content: {}]", mime_type),
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

fn bounded_snippet(text: String, max_chars: usize) -> String {
    let total_chars = text.chars().count();
    if total_chars <= max_chars {
        text
    } else {
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{truncated}…")
    }
}

fn slice_text_chunk(
    text: &str,
    total_chars: usize,
    offset_chars: usize,
    max_chars: usize,
) -> (String, usize, bool, Option<usize>) {
    let safe_offset = offset_chars.min(total_chars);

    if max_chars == 0 {
        let has_more = safe_offset < total_chars;
        let next_offset = has_more.then_some(safe_offset);
        return (String::new(), 0, has_more, next_offset);
    }

    if safe_offset == total_chars {
        return (String::new(), 0, false, None);
    }

    let mut start_byte = text.len();
    let mut end_byte = text.len();
    let mut chunk_length = 0usize;
    let mut started = false;

    for (char_index, (byte_index, ch)) in text.char_indices().enumerate() {
        if char_index == safe_offset {
            start_byte = byte_index;
            end_byte = byte_index;
            started = true;
        }

        if started && chunk_length < max_chars {
            end_byte = byte_index + ch.len_utf8();
            chunk_length += 1;

            if chunk_length == max_chars {
                break;
            }
        }
    }

    let chunk = text[start_byte..end_byte].to_string();
    let next_char_offset = safe_offset.saturating_add(chunk_length);
    let has_more = next_char_offset < total_chars;
    let next_offset = has_more.then_some(next_char_offset);
    (chunk, chunk_length, has_more, next_offset)
}

async fn resolve_allowed_session_ids(
    session_id: Option<&str>,
    agent_id: Option<&str>,
    from: Option<i64>,
    to: Option<i64>,
) -> Result<HashSet<String>, String> {
    let session_repo = get_session_repository();

    if let Some(session_id) = session_id {
        let session = session_repo
            .get_session(session_id)
            .await
            .map_err(|e| e.to_string())?;

        let Some(session) = session else {
            return Ok(HashSet::new());
        };

        let agent_matches = agent_id
            .map(|expected| {
                extract_agent_id(session.agent_config.as_deref()).as_deref() == Some(expected)
            })
            .unwrap_or(true);
        let reference_timestamp = session.last_message_at.unwrap_or(session.updated_at);

        return Ok(
            (agent_matches && timestamp_in_range(reference_timestamp, from, to))
                .then_some(session_id.to_string())
                .into_iter()
                .collect(),
        );
    }

    let sessions = session_repo
        .get_all_sessions()
        .await
        .map_err(|e| e.to_string())?;
    Ok(sessions
        .into_iter()
        .filter(|session| {
            let agent_matches = agent_id
                .map(|expected| {
                    extract_agent_id(session.agent_config.as_deref()).as_deref() == Some(expected)
                })
                .unwrap_or(true);
            let reference_timestamp = session.last_message_at.unwrap_or(session.updated_at);
            agent_matches && timestamp_in_range(reference_timestamp, from, to)
        })
        .map(|session| session.id)
        .collect())
}

/// Sanitizes a string for safe use as a plain-text Markdown table cell.
/// Collapses all line-ending variants (`\r\n`, `\n`, `\r`) into spaces and
/// replaces `|` with the HTML entity `&#124;` so it can't break the table
/// structure.
fn sanitize_cell(s: &str) -> String {
    s.replace("\r\n", " ")
        .replace(['\n', '\r'], " ")
        .replace('|', "&#124;")
}

fn render_list_text(page: &Page<HistorySessionItem>) -> String {
    let mut lines = vec![format!(
        "Found {} session(s) on page {} of {}.",
        page.total_items, page.page, page.total_pages
    )];

    if page.items.is_empty() {
        lines.push("No sessions matched the filters.".to_string());
    } else {
        lines.push(String::new());
        lines.push("| Name | ID | Status | Messages | Updated At |".to_string());
        lines.push("|---|---|---|---|---|".to_string());
        for session in &page.items {
            let name = sanitize_cell(session.name.as_deref().unwrap_or("Unnamed session"));
            let id = session.session_id.as_str();
            let status = sanitize_cell(&session.status);

            lines.push(format!(
                "| {} | `{}` | {} | {} | {} |",
                name, id, status, session.message_count, session.updated_at
            ));
        }

        if page.page < page.total_pages {
            lines.push(String::new());
            lines.push(format!(
                "*(Showing page {} of {}. Call this tool again with page: {} to see more)*",
                page.page,
                page.total_pages,
                page.page + 1
            ));
        } else if page.total_pages > 1 {
            lines.push(String::new());
            lines.push(format!(
                "*(Showing page {} of {}. End of results)*",
                page.page, page.total_pages
            ));
        }
    }

    lines.join("\n")
}

fn render_read_session_text(response: &HistorySessionReadResponse) -> String {
    let mut lines = vec![
        format!(
            "Session {} ({}) has {} message(s). Showing page {} of {}.",
            response
                .session
                .name
                .as_deref()
                .unwrap_or("Unnamed session"),
            response.session.session_id,
            response.messages.total_items,
            response.messages.page,
            response.messages.total_pages
        ),
        format!(
            "Status={} agentId={} lastMessageAt={}",
            response.session.status,
            response.session.agent_id.as_deref().unwrap_or("unknown"),
            response
                .session
                .last_message_at
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
    ];

    if response.messages.items.is_empty() {
        lines.push("This session has no messages on the requested page.".to_string());
    } else {
        lines.push(String::new());
        lines.push("| Message ID | Role | Created At | Chars | Preview |".to_string());
        lines.push("|---|---|---|---|---|".to_string());
        for message in &response.messages.items {
            let msg_id = message.message_id.as_str();
            let role = sanitize_cell(&message.role);
            let preview = sanitize_cell(&message.content_preview);

            lines.push(format!(
                "| `{}` | {} | {} | {} | {} |",
                msg_id, role, message.created_at, message.content_length, preview
            ));
        }

        if response.messages.page < response.messages.total_pages {
            lines.push(String::new());
            lines.push(format!(
                "*(Showing page {} of {}. Call this tool again with page: {} to see more)*",
                response.messages.page,
                response.messages.total_pages,
                response.messages.page + 1
            ));
        } else if response.messages.total_pages > 1 {
            lines.push(String::new());
            lines.push(format!(
                "*(Showing page {} of {}. End of results)*",
                response.messages.page, response.messages.total_pages
            ));
        }
    }

    lines.join("\n")
}

fn render_read_message_text(response: &HistoryMessageReadResponse) -> String {
    let mut lines = vec![
        format!(
            "Message {} from session {} [{}] chars {}-{} of {}.",
            response.message_id,
            response.session_id,
            response.role,
            response.chunk_offset,
            response.chunk_offset + response.chunk_length,
            response.total_chars
        ),
        String::new(),
        response.content_chunk.clone(),
    ];

    if response.has_more {
        lines.push(String::new());
        lines.push(format!(
            "More content remains. Next offset: {}.",
            response.next_offset.unwrap_or(response.total_chars)
        ));
    }

    lines.join("\n")
}

fn render_search_text(page: &Page<HistorySearchMatch>, caller_session_id: &str) -> String {
    let mut lines = vec![format!(
        "Found {} history match(es) on page {} of {}.",
        page.total_items, page.page, page.total_pages
    )];

    if page.items.is_empty() {
        lines.push("No matches found for the requested filters.".to_string());
    } else {
        lines.push(String::new());
        lines.push("| Session | Locality | Message | Role | Score | Chars | Snippet |".to_string());
        lines.push("|---|---|---|---|---|---|---|".to_string());
        for item in &page.items {
            let locality = if item.session_id == caller_session_id {
                "current-session"
            } else {
                "other-session"
            };
            let session_id = item.session_id.as_str();
            let message_id = item.message_id.as_str();
            let role = sanitize_cell(&item.role);
            let snippet = sanitize_cell(&item.snippet);

            lines.push(format!(
                "| `{}` | {} | `{}` | {} | {:.3} | {} | {} |",
                session_id, locality, message_id, role, item.score, item.content_length, snippet
            ));
        }

        if page.page < page.total_pages {
            lines.push(String::new());
            lines.push(format!(
                "*(Showing page {} of {}. Call this tool again with page: {} to see more)*",
                page.page,
                page.total_pages,
                page.page + 1
            ));
        } else if page.total_pages > 1 {
            lines.push(String::new());
            lines.push(format!(
                "*(Showing page {} of {}. End of results)*",
                page.page, page.total_pages
            ));
        }
    }

    lines.join("\n")
}
