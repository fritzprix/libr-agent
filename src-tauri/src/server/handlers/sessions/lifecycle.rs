//! Session mutation and lifecycle handlers (#2888): patch, resume,
//! terminate, and delete.

use crate::agent::AgentSessionManager;
use crate::utils::session_id::display_session_id;
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::super::helpers::{
    open_gate, resolve_error_reply, resolve_http_session_ref, resolve_session_ref_in,
};
use super::super::types::{ErrorResponse, PatchSessionRequest, PatchSessionResponse};
use crate::agent::session_manager::execution_mode::set_execution_mode_in;
use crate::agent::state::AgentSession;
use crate::execution_mode::ExecutionMode;
use crate::repositories::SessionRepository;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub async fn patch_session(
    id: String,
    manager: Arc<AgentSessionManager>,
    body: PatchSessionRequest,
) -> Result<impl Reply, Rejection> {
    Ok(patch_session_reply(
        &id,
        body,
        manager.active_sessions().as_ref(),
        &manager.repositories().session,
        Some(manager.app_handle()),
    )
    .await)
}

pub(super) async fn patch_session_reply(
    id: &str,
    body: PatchSessionRequest,
    active_sessions: &RwLock<HashMap<String, AgentSession>>,
    session_repo: &Arc<dyn SessionRepository>,
    app_handle: Option<&tauri::AppHandle>,
) -> warp::reply::WithStatus<warp::reply::Json> {
    let id = match resolve_session_ref_in(session_repo.as_ref(), id).await {
        Ok(id) => id,
        Err((status, error)) => return resolve_error_reply(status, error),
    };

    if body.execution_mode.is_none() && body.name.is_none() {
        return resolve_error_reply(
            StatusCode::BAD_REQUEST,
            "patch requires executionMode and/or name".to_string(),
        );
    }

    // Validate every field before mutating so a bad mode cannot leave a
    // half-applied rename (#2763 G1.2).
    let validated_mode = match body.execution_mode.as_ref() {
        Some(raw_mode) => match raw_mode.parse::<ExecutionMode>() {
            Ok(mode) => Some(mode),
            Err(error) => return resolve_error_reply(StatusCode::BAD_REQUEST, error),
        },
        None => None,
    };
    let validated_name = match body.name.as_ref() {
        Some(raw_name) => {
            let name = raw_name.trim();
            if name.is_empty() {
                return resolve_error_reply(
                    StatusCode::BAD_REQUEST,
                    "name must not be empty".to_string(),
                );
            }
            Some(name.to_string())
        }
        None => None,
    };

    let mut response = PatchSessionResponse {
        id: display_session_id(&id),
        execution_mode: None,
        name: None,
        resolved_approval_ids: Vec::new(),
    };

    if let Some(mode) = validated_mode {
        match set_execution_mode_in(active_sessions, session_repo, app_handle, &id, mode).await {
            Ok(resolved_approval_ids) => {
                response.execution_mode = Some(mode);
                response.resolved_approval_ids = resolved_approval_ids;
            }
            Err(error) => return resolve_error_reply(StatusCode::INTERNAL_SERVER_ERROR, error),
        }
    }

    if let Some(name) = validated_name {
        if let Err(error) = session_repo.update_name(&id, name.clone()).await {
            return resolve_error_reply(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
        }
        response.name = Some(name);
    }

    warp::reply::with_status(warp::reply::json(&response), StatusCode::OK)
}

/// POST /api/sessions/:id/resume
/// Loads a paused session into memory and resumes workflow without adding any new message.
/// Used by awaitAgent crash recovery to avoid injecting garbage user messages.
pub async fn resume_session_workflow(
    id: String,
    manager: Arc<AgentSessionManager>,
    attachment: Option<String>,
) -> Result<impl Reply, Rejection> {
    // Resuming is opening over this backend; the header is refused here and
    // the route claims nothing over HTTP (#2854, #2856).
    let id = match open_gate(
        &id,
        attachment.as_deref(),
        manager.repositories().session.as_ref(),
    )
    .await
    {
        Ok(id) => id,
        Err(refusal) => return Ok(refusal),
    };

    // Reload the session (runtime + MCP proxy), then resume from existing messages.
    if let Err(error) = manager.resume_session_and_workflow(&id).await {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse { error }),
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
