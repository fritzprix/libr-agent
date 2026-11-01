use crate::agent::session_manager::approvals::{
    respond_channel_permission_in, respond_tool_approval_in,
};
use crate::agent::state::AgentSession;
use crate::agent::AgentSessionManager;
use crate::mcp::types::ChannelNotification;
use crate::repositories::SessionRepository;
use crate::utils::session_id::display_session_id;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::{http::StatusCode, Rejection, Reply};

use super::helpers::{resolve_error_reply, resolve_http_session_ref, resolve_session_ref_in};
use super::types::{
    AutoRouteChannelResponse, ChannelPermissionRequestBody, ChannelPermissionResponse,
    ErrorResponse, InjectChannelRequest, SendMessageResponse,
};

pub async fn inject_channel_message(
    id: String,
    manager: Arc<AgentSessionManager>,
    body: InjectChannelRequest,
) -> Result<impl Reply, Rejection> {
    let id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    match manager.get_session(&id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Session not found: {}", id),
                }),
                StatusCode::NOT_FOUND,
            ));
        }
        Err(error) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Failed to validate session: {}", error),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    }

    let (message_id, triggered) = match manager
        .inject_channel_notification(
            id.clone(),
            body.server_name.clone(),
            ChannelNotification {
                content: body.content,
                meta: body.meta,
            },
        )
        .await
    {
        Ok(triggered) => triggered,
        Err(error) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Failed to inject channel message: {}", error),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&SendMessageResponse {
            id: message_id,
            status: if triggered {
                "processed".to_string()
            } else {
                "queued".to_string()
            },
        }),
        StatusCode::OK,
    ))
}

pub async fn inject_channel_message_auto(
    manager: Arc<AgentSessionManager>,
    body: InjectChannelRequest,
) -> Result<impl Reply, Rejection> {
    let (target, message_id, triggered) = match manager
        .inject_channel_notification_auto(
            body.server_name.clone(),
            ChannelNotification {
                content: body.content,
                meta: body.meta,
            },
        )
        .await
    {
        Ok(result) => result,
        Err(error) => {
            let status = if error.contains("No active session") {
                StatusCode::NOT_FOUND
            } else if error.contains("Ambiguous active sessions") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };

            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse { error }),
                status,
            ));
        }
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&AutoRouteChannelResponse {
            id: message_id,
            session_id: display_session_id(&target.session_id),
            session_name: target.session_name,
            status: if triggered {
                "processed".to_string()
            } else {
                "queued".to_string()
            },
        }),
        StatusCode::OK,
    ))
}

pub async fn respond_channel_permission(
    id: String,
    manager: Arc<AgentSessionManager>,
    body: ChannelPermissionRequestBody,
) -> Result<impl Reply, Rejection> {
    Ok(respond_channel_permission_reply(
        &id,
        body,
        manager.active_sessions().as_ref(),
        &manager.repositories().session,
        Some(manager.app_handle()),
    )
    .await)
}

async fn respond_channel_permission_reply(
    id: &str,
    body: ChannelPermissionRequestBody,
    active_sessions: &RwLock<HashMap<String, AgentSession>>,
    session_repo: &Arc<dyn SessionRepository>,
    app_handle: Option<&tauri::AppHandle>,
) -> warp::reply::WithStatus<warp::reply::Json> {
    let id = match resolve_session_ref_in(session_repo.as_ref(), id).await {
        Ok(id) => id,
        Err((status, error)) => return resolve_error_reply(status, error),
    };

    let approved =
        match crate::agent::tool_approvals::parse_channel_permission_behavior(&body.behavior) {
            Ok(approved) => approved,
            Err(error) => {
                return warp::reply::with_status(
                    warp::reply::json(&ErrorResponse { error }),
                    StatusCode::BAD_REQUEST,
                );
            }
        };

    let request_id_opt = body
        .request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let tool_call_id_opt = body
        .tool_call_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let (resolved_request_id, tool_call_id) = match (request_id_opt, tool_call_id_opt) {
        (Some(request_id), _) => {
            match respond_channel_permission_in(
                active_sessions,
                app_handle,
                &id,
                &request_id,
                approved,
            )
            .await
            {
                Ok(tool_call_id) => (Some(request_id), tool_call_id),
                Err(error) => {
                    let status = if error.contains("Session not found")
                        || error.contains("Pending approval not found")
                    {
                        StatusCode::NOT_FOUND
                    } else {
                        StatusCode::INTERNAL_SERVER_ERROR
                    };
                    return warp::reply::with_status(
                        warp::reply::json(&ErrorResponse { error }),
                        status,
                    );
                }
            }
        }
        (None, Some(tool_call_id)) => {
            match respond_tool_approval_in(
                active_sessions,
                app_handle,
                &id,
                &tool_call_id,
                approved,
            )
            .await
            {
                Ok(request_id) => (request_id, tool_call_id),
                Err(error) => {
                    let status = if error.contains("Session not found")
                        || error.contains("Pending approval not found")
                    {
                        StatusCode::NOT_FOUND
                    } else {
                        StatusCode::INTERNAL_SERVER_ERROR
                    };
                    return warp::reply::with_status(
                        warp::reply::json(&ErrorResponse { error }),
                        status,
                    );
                }
            }
        }
        (None, None) => {
            return warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: "Either 'requestId' or 'toolCallId' must be provided".to_string(),
                }),
                StatusCode::BAD_REQUEST,
            );
        }
    };

    warp::reply::with_status(
        warp::reply::json(&ChannelPermissionResponse {
            request_id: resolved_request_id,
            tool_call_id,
            approved,
        }),
        StatusCode::OK,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::context::registry::ContextRegistry;
    use crate::agent::state::{PendingApprovalData, PendingApprovalKind};
    use crate::repositories::{SessionMetadata, SqliteSessionRepository};
    use migration::{Migrator, MigratorTrait};
    use tokio::sync::oneshot;

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

    async fn reply_to_json(
        reply: warp::reply::WithStatus<warp::reply::Json>,
    ) -> (StatusCode, serde_json::Value) {
        let response = reply.into_response();
        let status = response.status();
        let body = warp::hyper::body::to_bytes(response.into_body())
            .await
            .expect("reply body should be readable");
        let json = serde_json::from_slice(&body).expect("reply body should be JSON");
        (status, json)
    }

    #[tokio::test]
    async fn respond_channel_permission_resolves_pending_approval_by_tool_call_id() {
        let repo = sqlite_session_repo().await;
        let id = "1122334455";
        let metadata = SessionMetadata::test_fixture(id);
        repo.upsert_session(&metadata)
            .await
            .expect("session should be stored");

        let agent_session = AgentSession::new(metadata, Arc::new(ContextRegistry::new()), None);
        let (tx, rx) = oneshot::channel();
        agent_session.pending_approvals.write().await.insert(
            "call-by-id-1".to_string(),
            PendingApprovalData {
                sender: tx,
                tool_name: "workspace__runShell".to_string(),
                arguments: r#"{"command":"ls"}"#.to_string(),
                approval_kind: PendingApprovalKind::Standard,
                request_id: None,
                description: Some("Execute shell command".to_string()),
                input_preview: Some("ls".to_string()),
            },
        );
        let active_sessions = RwLock::new(HashMap::from([(id.to_string(), agent_session)]));

        let (status, json) = reply_to_json(
            respond_channel_permission_reply(
                id,
                ChannelPermissionRequestBody {
                    request_id: None,
                    tool_call_id: Some("call-by-id-1".to_string()),
                    behavior: "allow".to_string(),
                },
                &active_sessions,
                &repo,
                None,
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["toolCallId"], "call-by-id-1");
        assert_eq!(json["approved"], true);
        assert_eq!(json["requestId"], serde_json::Value::Null);
        assert_eq!(rx.await, Ok(true));
    }

    #[tokio::test]
    async fn respond_channel_permission_rejects_missing_request_id_and_tool_call_id() {
        let repo = sqlite_session_repo().await;
        let id = "1122334455";
        repo.upsert_session(&SessionMetadata::test_fixture(id))
            .await
            .expect("session should be stored");
        let active_sessions = RwLock::new(HashMap::new());

        let (status, json) = reply_to_json(
            respond_channel_permission_reply(
                id,
                ChannelPermissionRequestBody {
                    request_id: None,
                    tool_call_id: None,
                    behavior: "allow".to_string(),
                },
                &active_sessions,
                &repo,
                None,
            )
            .await,
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(json["error"]
            .as_str()
            .unwrap_or_default()
            .contains("requestId"));
    }
}
