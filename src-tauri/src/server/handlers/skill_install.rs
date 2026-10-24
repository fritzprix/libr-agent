use crate::agent::AgentSessionManager;
use crate::server::skill_install::{stage_files, SkillInstallRequest};
use crate::server::wake::verify_wake_token;
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::types::ErrorResponse;

fn reply(
    status: StatusCode,
    body: serde_json::Value,
) -> warp::reply::WithStatus<warp::reply::Json> {
    warp::reply::with_status(warp::reply::json(&body), status)
}

fn error_reply(status: StatusCode, error: &str) -> warp::reply::WithStatus<warp::reply::Json> {
    reply(
        status,
        serde_json::to_value(ErrorResponse {
            error: error.to_string(),
        })
        .unwrap_or_default(),
    )
}

/// `POST /api/sessions/:id/skills/install/:token` — install one user skill.
///
/// The token is checked before the body is parsed. The skill is staged in a
/// scratch directory and installed through the regular import path, which
/// validates SKILL.md again. The token is never echoed or logged.
pub async fn install_skill(
    id: String,
    token: String,
    manager: Arc<AgentSessionManager>,
    body: warp::hyper::body::Bytes,
) -> Result<impl Reply, Rejection> {
    if !verify_wake_token(&id, &token) {
        return Ok(error_reply(
            StatusCode::UNAUTHORIZED,
            "invalid or expired session token",
        ));
    }
    match manager.get_session(&id).await {
        Ok(Some(_)) => {}
        Ok(None) => return Ok(error_reply(StatusCode::GONE, "session has ended")),
        Err(error) => {
            return Ok(error_reply(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("failed to validate session: {error}"),
            ))
        }
    }
    let request = match serde_json::from_slice::<SkillInstallRequest>(&body) {
        Ok(request) => request,
        Err(error) => {
            return Ok(error_reply(
                StatusCode::BAD_REQUEST,
                &format!("invalid install request: {error}"),
            ))
        }
    };

    let staged = tokio::task::spawn_blocking(move || {
        let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
        stage_files(&request, dir.path())?;
        Ok::<_, String>(dir)
    })
    .await;
    let dir = match staged {
        Ok(Ok(dir)) => dir,
        Ok(Err(error)) => return Ok(error_reply(StatusCode::BAD_REQUEST, &error)),
        Err(error) => {
            return Ok(error_reply(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("staging task failed: {error}"),
            ))
        }
    };

    match crate::services::skill_service::import_user_skills(
        dir.path().to_string_lossy().into_owned(),
        true,
        None,
    )
    .await
    {
        Ok(result) => Ok(reply(
            StatusCode::OK,
            serde_json::to_value(result).unwrap_or_default(),
        )),
        Err(error) => Ok(error_reply(StatusCode::BAD_REQUEST, &error)),
    }
}
