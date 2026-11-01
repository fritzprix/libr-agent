use crate::agent::AgentSessionManager;
use crate::models::chat::MessageSource;
use crate::repositories::{SessionMetadata, SessionStatus};
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::helpers::{map_message_for_http, resolve_error_reply, resolve_http_session_ref};
use super::types::{ErrorResponse, GetMessagesQuery, SendMessageRequest, SendMessageResponse};

pub async fn get_messages(
    id: String,
    query: GetMessagesQuery,
    manager: Arc<AgentSessionManager>,
) -> Result<impl Reply, Rejection> {
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
    let limit = query.limit.unwrap_or(50);

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

    if let Ok(Some(session)) = manager.get_session(&id).await {
        if let Some(error) = setup_failure_error(&session) {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse { error }),
                StatusCode::CONFLICT,
            ));
        }
    }

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

/// Only a failed setup (Docker provisioning) stores a reason with `error`; a session in
/// `error` after a workflow failure has no reason and still accepts a retry message.
fn setup_failure_error(session: &SessionMetadata) -> Option<String> {
    match (&session.status, session.status_reason.as_deref()) {
        (SessionStatus::Error, Some(reason)) => Some(format!(
            "Session status is 'error' and it cannot run messages: {reason}"
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_setup_failure_refuses_messages() {
        let mut session = SessionMetadata::test_fixture("setup-failure-session");
        assert!(setup_failure_error(&session).is_none());

        session.status = SessionStatus::Error;
        assert!(
            setup_failure_error(&session).is_none(),
            "a workflow error without a reason accepts a retry"
        );

        session.status_reason = Some("Invalid Docker workspace config: home".to_string());
        let error = setup_failure_error(&session).expect("setup failure refuses messages");
        assert!(error.contains("'error'"));
        assert!(error.contains("Invalid Docker workspace config: home"));
    }
}
