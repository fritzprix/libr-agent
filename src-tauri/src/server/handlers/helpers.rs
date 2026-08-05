pub use crate::services::agent_service::lineage_store;

use crate::agent::types::CreateSessionResponse;
use crate::models::chat::Message;
use crate::repositories::session_repository::SessionRepository;
use crate::repositories::SessionMetadata;
use crate::utils::session_id::{display_session_id, resolve_session_id_among, SessionIdResolve};
use warp::http::StatusCode;

use super::types::ErrorResponse;

/// Collect all stored session ids for HTTP alias resolution (global scope).
pub async fn collect_session_id_candidates() -> Result<Vec<String>, String> {
    let repo = crate::state::get_session_repository();
    let sessions = repo
        .get_all_sessions()
        .await
        .map_err(|e| format!("Failed to list sessions: {}", e))?;
    Ok(sessions.into_iter().map(|session| session.id).collect())
}

/// Resolve a path/body session reference to the stored session id.
///
/// Accepts full storage ids, bare short tokens, or optional `session-{short}` forms.
/// Ambiguous aliases return HTTP 400; missing refs return 404.
pub async fn resolve_http_session_ref(input_ref: &str) -> Result<String, (StatusCode, String)> {
    let candidates = collect_session_id_candidates()
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error))?;
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
