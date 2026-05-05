use crate::agent::AgentSessionManager;
use crate::models::chat::MessageSource;
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::types::{ErrorResponse, GetMessagesQuery, SendMessageRequest, SendMessageResponse};

pub async fn get_messages(
    id: String,
    query: GetMessagesQuery,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    // Validate session existence before fetching messages.
    // Without this check the message repo silently returns an empty list
    // for any unknown session ID, masking bugs.
    match manager.get_session(&id).await {
        Ok(None) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Session not found: {}", id),
                }),
                StatusCode::NOT_FOUND,
            ))
        }
        Err(e) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse {
                    error: format!("Failed to validate session: {}", e),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
        Ok(Some(_)) => {} // Session exists, proceed
    }

    let repo = crate::state::get_message_repository();
    use crate::repositories::message_repository::MessageRepository;
    let limit = query.limit.unwrap_or(50);

    match repo.get_messages_by_session(&id, limit).await {
        Ok(messages) => Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::json!({ "messages": messages })),
            StatusCode::OK,
        )),
        Err(e) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Failed to fetch messages: {}", e),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

pub async fn send_message(
    id: String,
    manager: Arc<AgentSessionManager>,
    body: SendMessageRequest,
) -> Result<impl Reply, Rejection> {
    match crate::services::AgentService::send_message_to_session(
        &manager,
        &id,
        body.content,
        body.source.or(Some(MessageSource::Api)),
    )
    .await
    {
        Ok(response) => Ok(warp::reply::with_status(
            warp::reply::json(&SendMessageResponse {
                id: response.message_id,
                status: response.status,
            }),
            StatusCode::OK,
        )),
        Err(error) if error.contains("Session not found:") => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse { error }),
            StatusCode::NOT_FOUND,
        )),
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorResponse {
                error: format!("Failed to process message: {}", error),
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}
