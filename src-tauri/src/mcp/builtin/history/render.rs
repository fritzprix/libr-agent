use super::types::{
    HistoryMessageReadResponse, HistorySearchMatch, HistorySessionItem, HistorySessionReadResponse,
};
use crate::utils::pagination::Page;

/// Render the list-sessions result as a markdown table text.
pub fn render_list_text(page: &Page<HistorySessionItem>) -> String {
    let mut lines = vec![format!(
        "Found {} session(s) on page {} of {}.",
        page.total_items, page.page, page.total_pages
    )];

    if page.items.is_empty() {
        lines.push("No sessions matched the filters.".to_string());
    } else {
        lines.push(String::new());
        lines.push("| Name | Session ID | Status | Messages | Updated At |".to_string());
        lines.push("|---|---|---|---|---|".to_string());
        for session in &page.items {
            let name = session
                .name
                .as_deref()
                .unwrap_or("Unnamed session")
                .replace('|', "\\|")
                .replace('\n', " ");
            lines.push(format!(
                "| {} | `{}` | {} | {} | {} |",
                name, session.session_id, session.status, session.message_count, session.updated_at
            ));
        }
    }

    lines.join("\n")
}

/// Render the read-session result as a markdown table text.
pub fn render_read_session_text(response: &HistorySessionReadResponse) -> String {
    let order_label = if response.order == "desc" {
        "newest first"
    } else {
        "oldest first"
    };
    let mut lines = vec![
        format!(
            "Session {} (`{}`) has {} message(s). Showing page {} of {} (order={}, {}).",
            response
                .session
                .name
                .as_deref()
                .unwrap_or("Unnamed session"),
            response.session.session_id,
            response.messages.total_items,
            response.messages.page,
            response.messages.total_pages,
            response.order,
            order_label
        ),
        format!(
            "Status={} agentId={} lastMessageAt={} totalPages={}",
            response.session.status,
            response.session.agent_id.as_deref().unwrap_or("unknown"),
            response
                .session
                .last_message_at
                .map(|value: i64| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            response.messages.total_pages
        ),
    ];

    if response.messages.items.is_empty() {
        lines.push("This session has no messages on the requested page.".to_string());
    } else {
        lines.push(String::new());
        lines.push("| Message ID | Role | Created At | Chars | Preview |".to_string());
        lines.push("|---|---|---|---|---|".to_string());
        for message in &response.messages.items {
            let preview = message
                .content_preview
                .replace('|', "\\|")
                .replace('\n', " ");
            lines.push(format!(
                "| `{}` | {} | {} | {} | {} |",
                message.message_id,
                message.role,
                message.created_at,
                message.content_length,
                preview
            ));
        }
    }

    lines.join("\n")
}

/// Render the read-message result as plain text with chunk metadata.
pub fn render_read_message_text(response: &HistoryMessageReadResponse) -> String {
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

/// Render the search-history result as a markdown table text.
pub fn render_search_text(page: &Page<HistorySearchMatch>, caller_session_id: &str) -> String {
    let mut lines = vec![format!(
        "Found {} history match(es) on page {} of {}.",
        page.total_items, page.page, page.total_pages
    )];

    if page.items.is_empty() {
        lines.push("No matches found for the requested filters.".to_string());
    } else {
        lines.push(String::new());
        lines.push(
            "| Session ID | Locality | Message ID | Role | Score | Chars | Snippet |".to_string(),
        );
        lines.push("|---|---|---|---|---|---|---|".to_string());
        for item in &page.items {
            let locality = if item.session_id == caller_session_id {
                "current-session"
            } else {
                "other-session"
            };
            let snippet = item.snippet.replace('|', "\\|").replace('\n', " ");
            lines.push(format!(
                "| `{}` | {} | `{}` | {} | {:.3} | {} | {} |",
                item.session_id,
                locality,
                item.message_id,
                item.role,
                item.score,
                item.content_length,
                snippet
            ));
        }
    }

    lines.join("\n")
}
