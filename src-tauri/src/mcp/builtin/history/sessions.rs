use serde_json::{json, Value};

use super::args::{ListArgs, ReadSessionArgs};
use super::constants::*;
use super::helpers::{
    build_message_counts, normalize_page, normalize_page_size, normalize_read_session_order,
    parse_optional_status, parse_optional_timestamp, session_matches_filters,
    to_history_session_item,
};
use super::types::{HistorySessionItem, HistorySessionReadResponse};
use super::HistoryServer;
use crate::mcp::builtin::error_guidance::{
    guided_error, operation_failed_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use crate::repositories::{MessageRepository, SessionRepository};
use crate::state::{get_message_repository, get_session_repository};
use crate::utils::pagination::paginate_in_memory;

/// List historical sessions with pagination and lightweight filters.
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
    let text = super::render::render_list_text(&paged);

    Ok(SuccessHint::new(
        text,
        vec!["Use history__readSession(sessionId=\"...\") to inspect messages".to_string()],
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

/// Read one session's metadata plus a paginated message list (previews only).
pub async fn read_session(_server: &HistoryServer, args: Value) -> Result<MCPResult, String> {
    let args: ReadSessionArgs =
        serde_json::from_value(args).map_err(|e| format!("Invalid readSession arguments: {e}"))?;
    let page = normalize_page(args.page.unwrap_or(DEFAULT_LIST_PAGE));
    let page_size = normalize_page_size(args.page_size.unwrap_or(DEFAULT_MESSAGE_PAGE_SIZE));
    let order = match normalize_read_session_order(args.order.as_deref()) {
        Ok(order) => order,
        Err(message) => {
            return Ok(
                guided_error(ErrorCategory::InvalidInput, message, ToolGroup::Agent)
                    .guidance(vec![
                        "Use order=\"desc\" for newest messages first (default)".to_string(),
                        "Use order=\"asc\" for oldest-first chronological paging".to_string(),
                    ])
                    .to_mcp_result(),
            );
        }
    };
    let newest_first = order == "desc";

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
                    "Use history__listSessions() to find a valid session ID".to_string(),
                    "Retry history__readSession with a copied sessionId from history__listSessions()".to_string(),
                ],
                ToolGroup::Agent,
            ));
        }
    };

    let message_page = message_repo
        .get_page_ordered(&args.session_id, page, page_size, newest_first)
        .await
        .map_err(|e| e.to_string())?;
    let total_messages = message_page.total_items;
    let messages = super::helpers::map_message_page(message_page);

    let response = HistorySessionReadResponse {
        session: super::helpers::to_history_session_item_with_count(session, total_messages),
        messages,
        order: order.to_string(),
    };

    let text = super::render::render_read_session_text(&response);

    Ok(SuccessHint::new(
        text,
        vec![
            "Use history__readMessage(messageId=\"...\") to page through a large message body"
                .to_string(),
        ],
    )
    .to_mcp_result_with_data(Some(json!(response))))
}
