use crate::agent::AgentSessionManager;
use crate::mcp::types::ChannelNotification;
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::types::{ErrorResponse, InjectChannelRequest, SendMessageResponse};

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
