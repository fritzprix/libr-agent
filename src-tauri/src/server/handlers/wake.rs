use crate::agent::AgentSessionManager;
use crate::models::chat::MessageSource;
use crate::server::wake::{verify_wake_token, WakePayload};
use std::sync::Arc;
use warp::{http::StatusCode, Rejection, Reply};

use super::types::{ErrorResponse, SendMessageResponse};

fn error_reply(status: StatusCode, error: &str) -> warp::reply::WithStatus<warp::reply::Json> {
    warp::reply::with_status(
        warp::reply::json(&ErrorResponse {
            error: error.to_string(),
        }),
        status,
    )
}

/// Token check first (so unauthenticated callers learn nothing about the
/// payload or session), then payload parsing. An empty body is a valid wake.
fn authorize_wake(id: &str, token: &str, body: &[u8]) -> Result<WakePayload, (StatusCode, String)> {
    if !verify_wake_token(id, token) {
        return Err((
            StatusCode::UNAUTHORIZED,
            "invalid or expired wake token".to_string(),
        ));
    }
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(WakePayload::default());
    }
    serde_json::from_slice::<WakePayload>(body).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid wake payload: {error}"),
        )
    })
}

/// `POST /api/sessions/:id/wake/:token` — process-completion callback.
///
/// The token is never echoed or logged. A busy session queues the message
/// like any other Session API message.
pub async fn wake_session(
    id: String,
    token: String,
    manager: Arc<AgentSessionManager>,
    body: warp::hyper::body::Bytes,
) -> Result<impl Reply, Rejection> {
    let payload = match authorize_wake(&id, &token, &body) {
        Ok(payload) => payload,
        Err((status, error)) => return Ok(error_reply(status, &error)),
    };

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

    match crate::services::AgentService::send_message_to_session(
        &manager,
        &id,
        payload.to_message(),
        Some(MessageSource::Api),
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
        Err(error) if error.contains("Session not found:") => {
            Ok(error_reply(StatusCode::GONE, "session has ended"))
        }
        Err(error) => Ok(error_reply(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("failed to deliver wake callback: {error}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::wake::{revoke_wake_token, wake_token_for};

    #[test]
    fn rejects_wrong_or_revoked_token_before_parsing() {
        let id = "wake-handler-test-auth";
        let token = wake_token_for(id);

        let (status, error) = authorize_wake(id, "not-the-token", b"not json").unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(!error.contains(&token));

        revoke_wake_token(id);
        let (status, _) = authorize_wake(id, &token, b"{}").unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn accepts_empty_or_json_body_with_valid_token() {
        let id = "wake-handler-test-body";
        let token = wake_token_for(id);

        assert!(authorize_wake(id, &token, b"").unwrap().exit_code.is_none());
        let payload =
            authorize_wake(id, &token, br#"{"exitCode":1,"logPath":"build.log"}"#).unwrap();
        assert_eq!(payload.exit_code, Some(1));
        assert_eq!(payload.log_path.as_deref(), Some("build.log"));

        let (status, _) = authorize_wake(id, &token, b"{oops").unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        revoke_wake_token(id);
    }
}
