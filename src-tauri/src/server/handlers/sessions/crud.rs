//! Session CRUD and query handlers (#2888): create, get, open, list, the
//! status snapshot body, and child sessions.

use crate::agent::AgentSessionManager;
use crate::models::chat::MessageSource;
use crate::repositories::SessionRepository;
use crate::utils::session_id::display_session_id;
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::super::helpers::{
    lineage_store, map_create_session_response_for_http, map_session_metadata_for_http, open_gate,
    resolve_create_session_body_refs, resolve_error_reply, resolve_http_session_ref,
    resolve_session_ref_in,
};
use super::super::types::{
    ChildSessionsResponse, CreateSessionRequest, ErrorResponse, ListSessionsQuery,
    ListSessionsResponse, SessionListItem, SessionStatusResponse,
};
use crate::agent::session_manager::approvals::list_pending_approvals_in;
use crate::agent::state::AgentSession;
use crate::repositories::session_repository::SessionListCursor;
use crate::server::session_ownership;
use std::collections::HashMap;
use tokio::sync::RwLock;

// Helpers that the test module (a sibling of `crud`/`lifecycle`) exercises
// directly; `pub(super)` keeps them visible only inside this module tree.
pub(super) fn classify_spawn_error(err: &str) -> (StatusCode, String) {
    if err.contains("Assistant not found") {
        (StatusCode::NOT_FOUND, err.to_string())
    } else if err.contains("Failed to fetch assistant") {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "An internal database error occurred while fetching the assistant details.".to_string(),
        )
    } else if err.contains("limit exceeded")
        || err.contains("must be absolute")
        || err.contains("restricted system directory")
        || err.contains("must be a directory")
        || err.contains("not accessible")
        || err.contains("Invalid assistant configuration")
        || err.contains("Invalid Docker workspace config")
    {
        (StatusCode::BAD_REQUEST, err.to_string())
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
    }
}

pub async fn create_session(
    manager: Arc<AgentSessionManager>,
    body: CreateSessionRequest,
    attachment: Option<String>,
) -> Result<impl Reply, Rejection> {
    // A fabricated attachment header is refused before spawning, so a bad
    // header never leaves a session behind (#2854). HTTP callers never
    // claim, and a brand-new session has no owner to refuse (#2856).
    if let Err((status, error)) = session_ownership::reject_attachment_header(attachment.as_deref())
    {
        return Ok(resolve_error_reply(status, error));
    }
    let body = match resolve_create_session_body_refs(body).await {
        Ok(body) => body,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    match crate::services::AgentService::spawn_agent_with_source(
        &manager,
        body,
        Some(MessageSource::Api),
    )
    .await
    {
        Ok(response) => Ok(warp::reply::with_status(
            warp::reply::json(&map_create_session_response_for_http(response)),
            StatusCode::CREATED,
        )),
        Err(e) => {
            let (status, sanitized_err) = classify_spawn_error(&e);
            Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: sanitized_err,
                }),
                status,
            ))
        }
    }
}

/// `GET /api/sessions/:id` — a metadata read. It never claims ownership and
/// never refuses because of ownership (#2759); claiming happens only on the
/// GUI/Tauri open paths, never over HTTP (#2856). The attachment header is
/// refused with `400` on the routes that read it (#2854).
pub async fn get_session(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    Ok(get_session_reply(
        &id,
        Some(manager.active_sessions().as_ref()),
        &manager.repositories().session,
    )
    .await)
}

pub(super) async fn get_session_reply(
    id: &str,
    active_sessions: Option<&RwLock<HashMap<String, AgentSession>>>,
    session_repo: &Arc<dyn SessionRepository>,
) -> warp::reply::WithStatus<warp::reply::Json> {
    let id = match resolve_session_ref_in(session_repo.as_ref(), id).await {
        Ok(id) => id,
        Err((status, error)) => return resolve_error_reply(status, error),
    };

    match session_status_snapshot(&id, active_sessions, session_repo).await {
        Ok(Some(snapshot)) => {
            warp::reply::with_status(warp::reply::json(&snapshot), StatusCode::OK)
        }
        Ok(None) => warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: "Session not found".to_string(),
            }),
            StatusCode::NOT_FOUND,
        ),
        Err(e) => warp::reply::with_status(
            warp::reply::json(&ErrorResponse { error: e }),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    }
}

/// `POST /api/sessions/:id/open` — open the session in this backend's run.
/// `200` for headerless callers (open paths claim nothing over HTTP,
/// #2856); `400` when an `x-frontend-attachment` header is sent (#2854);
/// `404` when the session cannot be resolved.
pub async fn open_session(
    id: String,
    manager: Arc<AgentSessionManager>,
    attachment: Option<String>,
) -> Result<impl Reply, Rejection> {
    Ok(open_session_reply(&id, attachment.as_deref(), &manager.repositories().session).await)
}

pub(super) async fn open_session_reply(
    id: &str,
    attachment: Option<&str>,
    session_repo: &Arc<dyn SessionRepository>,
) -> warp::reply::WithStatus<warp::reply::Json> {
    if let Err(refusal) = open_gate(id, attachment, session_repo.as_ref()).await {
        return refusal;
    }
    warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "status": "open" })),
        StatusCode::OK,
    )
}

/// The `GET /api/sessions/:id` body for an exact session id, or `None` when
/// the session does not exist. Also the `status` payload of the event stream.
pub(in crate::server::handlers) async fn session_status_snapshot(
    id: &str,
    active_sessions: Option<&RwLock<HashMap<String, AgentSession>>>,
    session_repo: &Arc<dyn SessionRepository>,
) -> Result<Option<SessionStatusResponse>, String> {
    let Some(session) = crate::agent::lifecycle::get_session(session_repo, id).await? else {
        return Ok(None);
    };
    let pending_approvals = match active_sessions {
        Some(active) => list_pending_approvals_in(active, id).await,
        None => Vec::new(),
    };
    let pending_user_prompt = match active_sessions {
        Some(active) => {
            let active = active.read().await;
            match active.get(id) {
                Some(agent_session) => {
                    crate::agent::state::pending_user_prompt_summary(
                        &agent_session.pending_user_prompt,
                    )
                    .await
                }
                None => None,
            }
        }
        None => None,
    };
    Ok(Some(SessionStatusResponse {
        session: map_session_metadata_for_http(session),
        pending_approvals,
        pending_user_prompt,
    }))
}

const DEFAULT_SESSIONS_LIMIT: u64 = 50;
const MAX_SESSIONS_LIMIT: u64 = 200;

/// `GET /api/sessions` — sessions ordered by most recent activity.
///
/// Pass the `nextCursor` of one page as `cursorUpdatedAt` + `cursorId` to get
/// the next one.
pub async fn list_sessions(
    query: ListSessionsQuery,
    manager: Arc<AgentSessionManager>,
    attachment: Option<String>,
) -> Result<impl Reply, Rejection> {
    // The header is refused before reading anything, matching the other
    // Session API routes (#2854).
    if let Err((status, error)) = session_ownership::reject_attachment_header(attachment.as_deref())
    {
        return Ok(resolve_error_reply(status, error));
    }
    Ok(list_sessions_reply(query, &manager.repositories().session).await)
}

pub(super) async fn list_sessions_reply(
    query: ListSessionsQuery,
    session_repo: &Arc<dyn SessionRepository>,
) -> warp::reply::WithStatus<warp::reply::Json> {
    let limit = query.limit.unwrap_or(DEFAULT_SESSIONS_LIMIT);
    if !(1..=MAX_SESSIONS_LIMIT).contains(&limit) {
        return resolve_error_reply(
            StatusCode::BAD_REQUEST,
            format!("Invalid limit {limit}: must be between 1 and {MAX_SESSIONS_LIMIT}"),
        );
    }
    let cursor = match (query.cursor_updated_at, query.cursor_id) {
        (Some(updated_at), Some(id)) => Some(SessionListCursor { updated_at, id }),
        (None, None) => None,
        _ => {
            return resolve_error_reply(
                StatusCode::BAD_REQUEST,
                "cursorUpdatedAt and cursorId must be given together".to_string(),
            )
        }
    };

    match session_repo
        .list_sessions(
            cursor,
            limit,
            query.search.as_deref(),
            query.bookmarked.unwrap_or(false),
            query.status.as_deref(),
        )
        .await
    {
        Ok(page) => {
            // Ownership is keyed by the storage id, so collect ids before
            // the display-id rewrite. One batched liveness read flags every
            // session some live process holds — including this backend's own
            // run, which is what an external HTTP caller sees (#2856 G1.2).
            let ids: Vec<String> = page.items.iter().map(|meta| meta.id.clone()).collect();
            let held = session_ownership::live_owned_ids(&ids).await;
            let mut items = Vec::with_capacity(page.items.len());
            for meta in page.items {
                let open_elsewhere = held.contains(&meta.id);
                items.push(SessionListItem {
                    session: map_session_metadata_for_http(meta),
                    open_elsewhere,
                });
            }
            warp::reply::with_status(
                warp::reply::json(&ListSessionsResponse {
                    sessions: items,
                    next_cursor: page.next_cursor,
                }),
                StatusCode::OK,
            )
        }
        Err(e) => resolve_error_reply(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to list sessions: {e}"),
        ),
    }
}

pub async fn get_child_sessions(id: String) -> Result<impl Reply, Rejection> {
    let id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    let session_repo = crate::state::get_session_repository();
    use crate::repositories::session_repository::SessionRepository;

    let children = match session_repo.get_child_session_ids(&id).await {
        Ok(ids) => ids,
        Err(_) => {
            let store = lineage_store().read().await;
            store
                .iter()
                .filter_map(|(session_id, meta)| {
                    if meta.parent_session_id.as_deref() == Some(id.as_str()) {
                        Some(session_id.clone())
                    } else {
                        None
                    }
                })
                .collect()
        }
    };

    let children_display: Vec<String> = children
        .iter()
        .map(|child_id| display_session_id(child_id))
        .collect();

    Ok(warp::reply::with_status(
        warp::reply::json(&ChildSessionsResponse {
            parent_session_id: display_session_id(&id),
            count: children_display.len(),
            children: children_display,
        }),
        StatusCode::OK,
    ))
}
