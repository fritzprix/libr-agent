use crate::agent::AgentSessionManager;
use crate::models::chat::MessageSource;
use crate::repositories::SessionRepository;
use crate::utils::session_id::display_session_id;
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::helpers::{
    lineage_store, map_create_session_response_for_http, map_session_metadata_for_http,
    resolve_create_session_body_refs, resolve_error_reply, resolve_http_session_ref,
    resolve_session_ref_in,
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
    Ok(get_session_reply(&id, &manager.repositories().session).await)
}

async fn get_session_reply(
    id: &str,
    session_repo: &Arc<dyn SessionRepository>,
) -> warp::reply::WithStatus<warp::reply::Json> {
    let id = match resolve_session_ref_in(session_repo.as_ref(), id).await {
        Ok(id) => id,
        Err((status, error)) => return resolve_error_reply(status, error),
    };

    match crate::agent::lifecycle::get_session(session_repo, &id).await {
        Ok(Some(session)) => warp::reply::with_status(
            warp::reply::json(&map_session_metadata_for_http(session)),
            StatusCode::OK,
        ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::{SessionMetadata, SessionStatus, SqliteSessionRepository};
    use migration::{Migrator, MigratorTrait};

    async fn sqlite_session_repo() -> Arc<dyn SessionRepository> {
        crate::lifecycle::database::register_sqlite_vec();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        Migrator::up(&db, None)
            .await
            .expect("migrations should run");
        Arc::new(SqliteSessionRepository::new(db))
    }

    async fn get_session_json(
        id: &str,
        repo: &Arc<dyn SessionRepository>,
    ) -> (StatusCode, serde_json::Value) {
        let response = get_session_reply(id, repo).await.into_response();
        let status = response.status();
        let body = warp::hyper::body::to_bytes(response.into_body())
            .await
            .expect("reply body should be readable");
        let json = serde_json::from_slice(&body).expect("reply body should be JSON");
        (status, json)
    }

    #[tokio::test]
    async fn get_session_returns_the_failure_reason_until_the_next_status_change() {
        let repo = sqlite_session_repo().await;
        let id = "status-reason-http-session";
        repo.upsert_session(&SessionMetadata::test_fixture(id))
            .await
            .expect("session should be stored");

        let (status, json) = get_session_json(id, &repo).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["statusReason"], serde_json::Value::Null);

        let reason = "Docker workspace setup failed: image 'missing:latest' was not found";
        repo.update_status(id, SessionStatus::Error, Some(reason))
            .await
            .expect("failure should be stored");
        let (status, json) = get_session_json(id, &repo).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["status"], "error");
        assert_eq!(json["statusReason"], reason);

        repo.update_status(id, SessionStatus::Idle, None)
            .await
            .expect("status change should be stored");
        let (_, json) = get_session_json(id, &repo).await;
        assert_eq!(json["status"], "idle");
        assert_eq!(json["statusReason"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn a_full_upsert_with_a_new_status_clears_the_failure_reason() {
        let repo = sqlite_session_repo().await;
        let id = "status-reason-upsert-session";
        let mut session = SessionMetadata::test_fixture(id);
        repo.upsert_session(&session)
            .await
            .expect("session should be stored");
        repo.update_status(id, SessionStatus::Error, Some("setup failed"))
            .await
            .expect("failure should be stored");

        session.status = SessionStatus::Idle;
        session.status_reason = None;
        repo.upsert_session(&session)
            .await
            .expect("upsert should be stored");

        let (_, json) = get_session_json(id, &repo).await;
        assert_eq!(json["status"], "idle");
        assert_eq!(json["statusReason"], serde_json::Value::Null);

        session.status = SessionStatus::Error;
        session.status_reason = Some("restart failed".to_string());
        repo.upsert_session(&session)
            .await
            .expect("upsert should be stored");
        let (_, json) = get_session_json(id, &repo).await;
        assert_eq!(json["statusReason"], "restart failed");
    }
}
