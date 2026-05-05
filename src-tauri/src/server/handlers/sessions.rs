use crate::agent::AgentSessionManager;
use crate::models::chat::MessageSource;
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::helpers::lineage_store;
use super::types::{ChildSessionsResponse, CreateSessionRequest, ErrorResponse};

pub async fn create_session(
    manager: Arc<AgentSessionManager>,
    body: CreateSessionRequest,
) -> Result<impl Reply, Rejection> {
    match crate::services::AgentService::spawn_agent_with_source(
        &manager,
        body,
        Some(MessageSource::Api),
    )
    .await
    {
        Ok(response) => Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::CREATED,
        )),
        Err(e) => {
            if e.contains("Assistant not found") {
                Ok(warp::reply::with_status(
                    warp::reply::json(&ErrorResponse { error: e }),
                    StatusCode::NOT_FOUND,
                ))
            } else if e.contains("limit exceeded")
                || e.contains("must be absolute")
                || e.contains("restricted system directory")
                || e.contains("must be a directory")
                || e.contains("not accessible")
            {
                Ok(warp::reply::with_status(
                    warp::reply::json(&ErrorResponse { error: e }),
                    StatusCode::BAD_REQUEST,
                ))
            } else {
                Ok(warp::reply::with_status(
                    warp::reply::json(&ErrorResponse { error: e }),
                    StatusCode::INTERNAL_SERVER_ERROR,
                ))
            }
        }
    }
}

pub async fn get_session(
    id: String,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    match manager.get_session(&id).await {
        Ok(Some(session)) => Ok(warp::reply::with_status(
            warp::reply::json(&session),
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
    match manager.terminate_session(id.clone()).await {
        Ok(_) => {
            lineage_store().write().await.remove(&id);
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

pub async fn get_child_sessions(id: String) -> Result<impl Reply, Rejection> {
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

    Ok(warp::reply::with_status(
        warp::reply::json(&ChildSessionsResponse {
            parent_session_id: id,
            count: children.len(),
            children,
        }),
        StatusCode::OK,
    ))
}
