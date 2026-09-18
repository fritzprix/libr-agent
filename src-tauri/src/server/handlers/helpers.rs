pub use crate::services::agent_service::lineage_store;

use crate::agent::types::CreateSessionResponse;
use crate::models::chat::Message;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionMetadata;
use crate::state::get_session_repository;
use crate::utils::session_id::{
    display_session_id, resolve_session_id_among, should_try_legacy_session_resolve,
    SessionIdResolve,
};
use warp::http::StatusCode;

use super::types::ErrorResponse;

/// Resolve a path/body session reference to the stored session id.
///
/// Exact match first; legacy short suffix / `session-{…}` as read-only fallback.
pub async fn resolve_http_session_ref(input_ref: &str) -> Result<String, (StatusCode, String)> {
    let repo = get_session_repository();
    if repo
        .get_session(input_ref)
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .is_some()
    {
        return Ok(input_ref.to_string());
    }

    if !should_try_legacy_session_resolve(input_ref) {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Session not found: {}", input_ref),
        ));
    }

    let candidates = repo
        .get_all_sessions()
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list sessions: {error}"),
            )
        })?
        .into_iter()
        .map(|session| session.id)
        .collect::<Vec<_>>();
    let candidate_refs: Vec<&str> = candidates.iter().map(|id| id.as_str()).collect();

    match resolve_session_id_among(candidate_refs, input_ref) {
        SessionIdResolve::Unique(id) => Ok(id.to_string()),
        SessionIdResolve::Missing => Err((
            StatusCode::NOT_FOUND,
            format!("Session not found: {}", input_ref),
        )),
        SessionIdResolve::Ambiguous(count) => Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Ambiguous session reference '{}': matches {} sessions. Use the full storage id.",
                input_ref, count
            ),
        )),
    }
}

pub fn map_optional_session_id(value: Option<&str>) -> Option<String> {
    value.map(display_session_id)
}

/// Rewrite session-identifying fields on metadata for external HTTP clients.
pub fn map_session_metadata_for_http(mut meta: SessionMetadata) -> SessionMetadata {
    meta.id = display_session_id(&meta.id);
    meta.parent_session_id = map_optional_session_id(meta.parent_session_id.as_deref());
    meta.lineage_id = map_optional_session_id(meta.lineage_id.as_deref());
    meta.org_root_session_id = map_optional_session_id(meta.org_root_session_id.as_deref());
    meta
}

/// Rewrite session-identifying fields on create responses for external HTTP clients.
pub fn map_create_session_response_for_http(
    mut response: CreateSessionResponse,
) -> CreateSessionResponse {
    response.id = display_session_id(&response.id);
    response.parent_session_id = map_optional_session_id(response.parent_session_id.as_deref());
    response.lineage_id = display_session_id(&response.lineage_id);
    response.org_root_session_id = map_optional_session_id(response.org_root_session_id.as_deref());
    response
}

/// Rewrite `sessionId` on chat messages for external HTTP clients.
pub fn map_message_for_http(mut message: Message) -> Message {
    message.session_id = display_session_id(&message.session_id);
    message
}

pub fn resolve_error_reply(
    status: StatusCode,
    error: String,
) -> warp::reply::WithStatus<warp::reply::Json> {
    warp::reply::with_status(warp::reply::json(&ErrorResponse { error }), status)
}

/// Resolve optional parent/org-root refs on create bodies before spawn.
pub async fn resolve_create_session_body_refs(
    mut body: crate::agent::types::CreateSessionRequest,
) -> Result<crate::agent::types::CreateSessionRequest, (StatusCode, String)> {
    if let Some(parent_ref) = body.parent_session_id.take() {
        body.parent_session_id = Some(resolve_http_session_ref(&parent_ref).await?);
    }
    if let Some(root_ref) = body.org_root_session_id.take() {
        body.org_root_session_id = Some(resolve_http_session_ref(&root_ref).await?);
    }
    Ok(body)
}
