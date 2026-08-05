use crate::agent::AgentSessionManager;
use crate::models::chat::MessageSource;
use crate::utils::session_id::display_session_id;
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::helpers::{
    lineage_store, map_create_session_response_for_http, map_session_metadata_for_http,
    resolve_create_session_body_refs, resolve_error_reply, resolve_http_session_ref,
};
use super::types::{ChildSessionsResponse, CreateSessionRequest, ErrorResponse};

fn classify_spawn_error(err: &str) -> (StatusCode, String) {
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
    {
        (StatusCode::BAD_REQUEST, err.to_string())
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
    }
}

pub async fn create_session(
    manager: Arc<AgentSessionManager>,
    body: CreateSessionRequest,
) -> Result<impl Reply, Rejection> {
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

pub async fn get_session(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    let id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    match manager.get_session(&id).await {
        Ok(Some(session)) => Ok(warp::reply::with_status(
            warp::reply::json(&map_session_metadata_for_http(session)),
            StatusCode::OK,
        )),
        Ok(None) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: "Session not found".to_string(),
            }),
            StatusCode::NOT_FOUND,
        )),
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse { error: e }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

/// POST /api/sessions/:id/resume
/// Loads a paused session into memory and resumes workflow without adding any new message.
/// Used by awaitAgent crash recovery to avoid injecting garbage user messages.
pub async fn resume_session_workflow(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    let id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    // Step 1: Load the session into active_sessions and recreate the MCP proxy
    if let Err(e) = manager.resume_session(&id).await {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Failed to resume session: {}", e),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        ));
    }

    // Step 2: Resume the workflow from existing messages (no new message injection)
    if let Err(e) = manager.resume_workflow(id.clone()).await {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Failed to resume workflow: {}", e),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        ));
    }

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "status": "resumed" })),
        StatusCode::OK,
    ))
}

pub async fn terminate_session(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    let id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    match manager.terminate_session(id.clone()).await {
        Ok(_) => {
            crate::services::agent_service::remove_lineage(&id).await;
            Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({ "success": true })),
                StatusCode::OK,
            ))
        }
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse { error: e }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

/// DELETE /api/sessions/:id — remove the session (and cascade children) entirely.
pub async fn delete_session(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    let id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    match manager.delete_session(id.clone()).await {
        Ok(deleted_ids) => {
            for deleted_id in &deleted_ids {
                crate::services::agent_service::remove_lineage(deleted_id).await;
            }
            let deleted_display: Vec<String> = deleted_ids
                .iter()
                .map(|deleted_id| display_session_id(deleted_id))
                .collect();
            Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({
                    "success": true,
                    "deletedIds": deleted_display,
                })),
                StatusCode::OK,
            ))
        }
        Err(e) => {
            let status = if e.contains("not found") || e.contains("Not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse { error: e }),
                status,
            ))
        }
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
