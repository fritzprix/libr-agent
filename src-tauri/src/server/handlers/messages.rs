use crate::agent::AgentSessionManager;
use crate::models::chat::MessageSource;
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::helpers::{map_message_for_http, resolve_error_reply, resolve_http_session_ref};
use super::types::{ErrorResponse, GetMessagesQuery, SendMessageRequest, SendMessageResponse};

const DEFAULT_MESSAGES_LIMIT: u64 = 50;
const MAX_MESSAGES_LIMIT: u64 = 1000;

fn resolve_messages_limit(limit: Option<u64>) -> Result<u64, (StatusCode, String)> {
    let limit = limit.unwrap_or(DEFAULT_MESSAGES_LIMIT);
    if (1..=MAX_MESSAGES_LIMIT).contains(&limit) {
        Ok(limit)
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid limit {}: must be between 1 and {}",
                limit, MAX_MESSAGES_LIMIT
            ),
        ))
    }
}

pub async fn get_messages(
    id: String,
    query: GetMessagesQuery,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
    let limit = match resolve_messages_limit(query.limit) {
        Ok(limit) => limit,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    let id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

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

    match repo.get_messages_by_session(&id, limit).await {
        Ok(messages) => {
            let messages: Vec<_> = messages.into_iter().map(map_message_for_http).collect();
            Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({ "messages": messages })),
                StatusCode::OK,
            ))
        }
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
    let id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    match crate::services::AgentService::send_message_to_session(
        &manager,
        &id,
        body.content,
        body.source.or(Some(MessageSource::Api)),
        false,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_messages_limit_defaults_to_50_and_accepts_bounds() {
        assert_eq!(resolve_messages_limit(None), Ok(50));
        assert_eq!(resolve_messages_limit(Some(1)), Ok(1));
        assert_eq!(resolve_messages_limit(Some(500)), Ok(500));
        assert_eq!(resolve_messages_limit(Some(1000)), Ok(1000));
    }

    #[test]
    fn resolve_messages_limit_rejects_zero_and_values_above_1000() {
        let (status, err) = resolve_messages_limit(Some(0)).unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(err.contains("between 1 and 1000"));

        let (status, err) = resolve_messages_limit(Some(1001)).unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(err.contains("between 1 and 1000"));

        let (status, _) = resolve_messages_limit(Some(u64::MAX)).unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
