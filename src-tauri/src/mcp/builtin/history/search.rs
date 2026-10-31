use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use super::args::SearchArgs;
use super::constants::*;
use super::helpers::{bounded_snippet, render_message_content};
use super::helpers::{
    normalize_page, normalize_page_size, parse_optional_timestamp, timestamp_in_range,
};
use super::types::HistorySearchMatch;
use super::HistoryServer;
use crate::mcp::builtin::error_guidance::{operation_failed_error, SuccessHint, ToolGroup};
use crate::mcp::types::MCPResult;
use crate::models::chat::Message;
use crate::repositories::{MessageRepository, SessionRepository};
use crate::services::message_service::MessageService;
use crate::state::{get_message_repository, get_session_repository};
use crate::utils::pagination::paginate_in_memory;

/// Search message history and return bounded snippets plus IDs for follow-up reads.
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
                    "Use history__listSessions() to find a valid session ID".to_string(),
                    "Retry history__searchHistory() with a copied sessionId from history__listSessions()".to_string(),
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

    if allowed_session_ids.is_empty() {
        let paged = paginate_in_memory(Vec::<HistorySearchMatch>::new(), page, page_size);
        let text = super::render::render_search_text(&paged, caller_session_id);

        return Ok(SuccessHint::new(
            text,
            vec![
                "Use history__listSessions() to inspect sessions that match your current filters"
                    .to_string(),
                "Relax sessionId/agentId/from/to filters and retry history__searchHistory()"
                    .to_string(),
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
        }))));
    }

    if let Some(session_id) = args.session_id.as_deref() {
        if !allowed_session_ids.contains(session_id) {
            return Ok(operation_failed_error(
                "Search History",
                &format!(
                    "Session '{}' did not match the provided filters",
                    session_id
                ),
                vec![
                    "Use history__listSessions() to confirm the session ID".to_string(),
                    "Relax agentId/from/to filters and try history__searchHistory() again"
                        .to_string(),
                ],
                ToolGroup::Agent,
            ));
        }
    }

    let scan_page_size = page
        .saturating_mul(page_size)
        .saturating_mul(10)
        .clamp(page_size, SEARCH_SCAN_LIMIT);
    let narrowed_session_id = args.session_id.clone().or_else(|| {
        if allowed_session_ids.len() == 1 {
            allowed_session_ids.iter().next().cloned()
        } else {
            None
        }
    });

    let raw_page =
        MessageService::search_messages(args.query.clone(), narrowed_session_id, 1, scan_page_size)
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
    let text = super::render::render_search_text(&paged, caller_session_id);

    Ok(SuccessHint::new(
        text,
        vec![
            "Use history__readSession(sessionId=\"...\") to inspect the surrounding conversation"
                .to_string(),
            "Use history__readMessage(messageId=\"...\") to expand a specific hit".to_string(),
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

/// Resolve which session IDs are allowed given optional filters.
pub async fn resolve_allowed_session_ids(
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
                crate::agent::extract_assistant_id_from_session(&session).as_deref()
                    == Some(expected)
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
                    crate::agent::extract_assistant_id_from_session(session).as_deref()
                        == Some(expected)
                })
                .unwrap_or(true);
            let reference_timestamp = session.last_message_at.unwrap_or(session.updated_at);
            agent_matches && timestamp_in_range(reference_timestamp, from, to)
        })
        .map(|session| session.id)
        .collect())
}
