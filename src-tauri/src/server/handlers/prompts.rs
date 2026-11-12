//! User-prompt answering for the release Session API.
//!
//! A session can block waiting on the user (`ui__askUser`, secret requests).
//! The waiting prompt — including its `requestId` and `kind` — is exposed on
//! `GET /api/sessions/:id` as `pendingUserPrompt`; clients answer or cancel
//! it through `POST /api/sessions/:id/channel/prompt` (API-key guarded).
//!
//! The planning logic is shared with the debug E2E control plane, which
//! re-exports [`UserPromptBody`], [`PlannedPromptAction`], and
//! [`plan_user_prompt_action`] from here.

use std::sync::Arc;

use serde::Deserialize;
use warp::{http::StatusCode, Rejection, Reply};

use crate::agent::session_manager::pending_user_prompt_in;
use crate::agent::state::{
    PendingUserPromptSummary, UserPromptAnswer, UserPromptKind, USER_PROMPT_GONE_MESSAGE,
};
use crate::agent::AgentSessionManager;

use super::helpers::{resolve_error_reply, resolve_http_session_ref};
use super::types::ErrorResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPromptBody {
    pub request_id: String,
    /// `answer` delivers `value`; `cancel` resolves the prompt unanswered.
    pub action: String,
    #[serde(default)]
    pub value: Option<String>,
}

/// What a validated user-prompt request asks the runtime to deliver.
#[derive(Debug, PartialEq, Eq)]
pub enum PlannedPromptAction {
    Answer(UserPromptAnswer),
    Cancel,
}

/// Pure dispatch decision behind [`respond_channel_prompt`]: matches the
/// waiting prompt against `requestId`, then maps `action`/`value` to the
/// answer to deliver. Errors carry `(status, message)` exactly as the
/// handler replies, so stale-requestId rejection, missing answer values,
/// and unknown actions are testable without a live `AgentSessionManager`.
pub fn plan_user_prompt_action(
    pending: Option<&PendingUserPromptSummary>,
    body: &UserPromptBody,
) -> Result<PlannedPromptAction, (StatusCode, String)> {
    let pending = match pending {
        Some(pending) if pending.request_id == body.request_id => pending,
        _ => return Err((StatusCode::NOT_FOUND, USER_PROMPT_GONE_MESSAGE.to_string())),
    };
    match body.action.as_str() {
        "answer" => {
            let Some(value) = body.value.clone() else {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Missing value for answer action".to_string(),
                ));
            };
            let answer = match pending.kind {
                UserPromptKind::Secret => UserPromptAnswer::Secret(value),
                UserPromptKind::Question => UserPromptAnswer::Question(value),
            };
            Ok(PlannedPromptAction::Answer(answer))
        }
        "cancel" => Ok(PlannedPromptAction::Cancel),
        other => Err((
            StatusCode::BAD_REQUEST,
            format!("Unknown user prompt action: {other}"),
        )),
    }
}

/// `POST /api/sessions/:id/channel/prompt` — answer or cancel the prompt
/// this session is waiting on.
pub async fn respond_channel_prompt(
    id: String,
    manager: Arc<AgentSessionManager>,
    body: UserPromptBody,
) -> Result<impl Reply, Rejection> {
    let session_id = match resolve_http_session_ref(&id).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(resolve_error_reply(status, error)),
    };

    let pending = pending_user_prompt_in(manager.as_ref(), &session_id).await;
    let planned = match plan_user_prompt_action(pending.as_ref(), &body) {
        Ok(planned) => planned,
        Err((status, error)) => {
            return Ok(warp::reply::with_status(
                warp::reply::json(&ErrorResponse { error }),
                status,
            ));
        }
    };

    let delivered = match planned {
        PlannedPromptAction::Answer(answer) => {
            match manager
                .answer_user_prompt(&session_id, &body.request_id, answer)
                .await
            {
                Ok(delivered) => delivered,
                Err(error) => {
                    return Ok(warp::reply::with_status(
                        warp::reply::json(&ErrorResponse { error }),
                        StatusCode::CONFLICT,
                    ));
                }
            }
        }
        PlannedPromptAction::Cancel => {
            match manager
                .cancel_user_prompt(&session_id, &body.request_id)
                .await
            {
                Ok(delivered) => delivered,
                Err(error) => {
                    return Ok(warp::reply::with_status(
                        warp::reply::json(&ErrorResponse { error }),
                        StatusCode::CONFLICT,
                    ));
                }
            }
        }
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "delivered": delivered })),
        StatusCode::OK,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::state::{UserPromptKind, UserPromptPayload};

    fn waiting(kind: UserPromptKind) -> PendingUserPromptSummary {
        PendingUserPromptSummary {
            request_id: "req-1".to_string(),
            kind,
            payload: match kind {
                UserPromptKind::Secret => UserPromptPayload::Secret {
                    variable_name: "TOKEN".to_string(),
                    label: "token".to_string(),
                    reuse_note: None,
                },
                UserPromptKind::Question => UserPromptPayload::Question {
                    questions: vec![crate::agent::state::UserQuestion {
                        id: "q1".to_string(),
                        question: "Pick one".to_string(),
                        description: None,
                        options: None,
                        multi_select: false,
                        allow_custom: true,
                    }],
                },
            },
            created_at: chrono::Utc::now(),
        }
    }

    fn body(request_id: &str, action: &str, value: Option<&str>) -> UserPromptBody {
        UserPromptBody {
            request_id: request_id.to_string(),
            action: action.to_string(),
            value: value.map(str::to_string),
        }
    }

    #[test]
    fn answer_secret_maps_to_secret_answer() {
        let planned = plan_user_prompt_action(
            Some(&waiting(UserPromptKind::Secret)),
            &body("req-1", "answer", Some("s3cret")),
        )
        .expect("should plan");
        assert_eq!(
            planned,
            PlannedPromptAction::Answer(UserPromptAnswer::Secret("s3cret".to_string()))
        );
    }

    #[test]
    fn stale_request_id_is_not_found() {
        let err = plan_user_prompt_action(
            Some(&waiting(UserPromptKind::Question)),
            &body("other", "answer", Some("x")),
        )
        .expect_err("stale request id must fail");
        assert_eq!(err.0, StatusCode::NOT_FOUND);
    }

    #[test]
    fn cancel_maps_to_cancel_action() {
        let planned = plan_user_prompt_action(
            Some(&waiting(UserPromptKind::Question)),
            &body("req-1", "cancel", None),
        )
        .expect("should plan");
        assert_eq!(planned, PlannedPromptAction::Cancel);
    }
}
