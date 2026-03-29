use crate::agent::AgentSessionManager;
use crate::mcp::types::ChannelNotification;
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::types::{
    AutoRouteChannelResponse, ChannelPermissionRequestBody, ChannelPermissionResponse,
    ErrorResponse, InjectChannelRequest, SendMessageResponse,
};

pub async fn inject_channel_message(
    id: String,
    manager: Arc<AgentSessionManager>,
    body: InjectChannelRequest,
) -> Result<impl Reply, Rejection> {
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

    let message_id = uuid::Uuid::new_v4().to_string();

    let triggered = match manager
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
    let message_id = uuid::Uuid::new_v4().to_string();

    let (target, triggered) = match manager
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
            session_id: target.session_id,
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
    let approved =
        match crate::agent::tool_approvals::parse_channel_permission_behavior(&body.behavior) {
            Ok(approved) => approved,
            Err(error) => {
                return Ok(warp::reply::with_status(
                    warp::reply::json(&ErrorResponse { error }),
                    StatusCode::BAD_REQUEST,
                ));
            }
        };

    let tool_call_id = match manager
        .respond_channel_permission(&id, &body.request_id, approved)
        .await
    {
        Ok(tool_call_id) => tool_call_id,
        Err(error) => {
            let status = if error.contains("Session not found")
                || error.contains("Pending approval not found")
            {
                StatusCode::NOT_FOUND
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
        warp::reply::json(&ChannelPermissionResponse {
            request_id: body.request_id,
            tool_call_id,
            approved,
        }),
        StatusCode::OK,
    ))
}
