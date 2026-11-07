//! E2E control plane handlers (debug builds only).
//!
//! Routes let an unattended harness observe and answer the two blocking
//! interactions a session can wait on:
//!
//! - the shared user prompt (#2543: `ui__askUser` / secret requests), and
//! - pending tool-approval requests.
//!
//! Every route sits behind [`capability_guard`], a pure filter that takes the
//! capability as a parameter — no env or process-global reads inside — so it
//! is directly unit-testable with `warp::test::request`:
//!
//! - no capability (control plane disabled) → `404 not found`,
//! - wrong or missing header → `403` via [`AccessDenied`].
//!
//! The route set itself only exists in debug binaries: `server/mod.rs` gates
//! the whole module behind `#[cfg(debug_assertions)]`.

use std::convert::Infallible;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use warp::http::StatusCode;
use warp::{Filter, Rejection, Reply};

use crate::agent::session_manager::{
    approvals::{list_pending_approvals_in, respond_tool_approval},
    pending_user_prompt_in, PendingApprovalSummary,
};
use crate::agent::state::{UserPromptAnswer, UserPromptKind, USER_PROMPT_GONE_MESSAGE};
use crate::agent::AgentSessionManager;
use crate::server::access_guard::AccessDenied;
use crate::server::e2e_control::token::{capability_token, header_matches};
use crate::server::handlers::helpers::resolve_http_session_ref;

/// Guard filter: 404 when the control plane is disabled, 403 when the
/// capability header is missing or wrong. Pure — the capability is passed in.
pub fn capability_guard(
    capability: Option<String>,
) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::header::headers_cloned()
        .and_then(move |headers: warp::http::HeaderMap| {
            let capability = capability.clone();
            async move {
                match capability.as_deref() {
                    None => Err(warp::reject::not_found()),
                    Some(expected) if header_matches(&headers, expected) => Ok(()),
                    Some(_) => Err(warp::reject::custom(AccessDenied {
                        reason: "missing or invalid e2e control capability",
                    })),
                }
            }
        })
        .untuple_one()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct E2eControlStatus {
    pub enabled: bool,
    pub debug_build: bool,
    pub loopback_only: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInteractionsSnapshot {
    pub session_id: String,
    pub pending_user_prompt:
        Option<crate::agent::state::PendingUserPromptSummary>,
    pub pending_approvals: Vec<PendingApprovalSummary>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPromptBody {
    pub request_id: String,
    /// `answer` delivers `value`; `cancel` resolves the prompt unanswered.
    pub action: String,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolApprovalBody {
    pub tool_call_id: String,
    pub approved: bool,
}

type JsonReply = warp::reply::WithStatus<warp::reply::Json>;

fn json_ok<T: Serialize>(body: &T) -> JsonReply {
    warp::reply::with_status(warp::reply::json(body), StatusCode::OK)
}

fn json_error(status: StatusCode, error: impl Into<String>) -> JsonReply {
    warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "error": error.into() })),
        status,
    )
}

async fn handle_status() -> Result<JsonReply, Infallible> {
    // Reaching this handler means the capability guard passed, i.e. the
    // control plane is enabled in this debug process.
    Ok(json_ok(&E2eControlStatus {
        enabled: true,
        debug_build: cfg!(debug_assertions),
        loopback_only: true,
    }))
}

async fn handle_get_interactions(
    session_ref: String,
    manager: Arc<AgentSessionManager>,
) -> Result<JsonReply, Infallible> {
    let session_id = match resolve_http_session_ref(&session_ref).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(json_error(status, error)),
    };
    if !manager.active_sessions().read().await.contains_key(&session_id) {
        return Ok(json_error(
            StatusCode::NOT_FOUND,
            format!("Session not found: {session_id}"),
        ));
    }
    let snapshot = SessionInteractionsSnapshot {
        session_id: session_id.clone(),
        pending_user_prompt: pending_user_prompt_in(manager.as_ref(), &session_id).await,
        pending_approvals: list_pending_approvals_in(manager.active_sessions(), &session_id).await,
    };
    Ok(json_ok(&snapshot))
}

async fn handle_user_prompt(
    session_ref: String,
    manager: Arc<AgentSessionManager>,
    body: UserPromptBody,
) -> Result<JsonReply, Infallible> {
    let session_id = match resolve_http_session_ref(&session_ref).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(json_error(status, error)),
    };

    let pending = match pending_user_prompt_in(manager.as_ref(), &session_id).await {
        Some(pending) if pending.request_id == body.request_id => pending,
        _ => return Ok(json_error(StatusCode::NOT_FOUND, USER_PROMPT_GONE_MESSAGE)),
    };

    let delivered = match body.action.as_str() {
        "answer" => {
            let Some(value) = body.value else {
                return Ok(json_error(
                    StatusCode::BAD_REQUEST,
                    "Missing value for answer action",
                ));
            };
            let answer = match pending.kind {
                UserPromptKind::Secret => UserPromptAnswer::Secret(value),
                UserPromptKind::Question => UserPromptAnswer::Question(value),
            };
            match manager
                .answer_user_prompt(&session_id, &body.request_id, answer)
                .await
            {
                Ok(delivered) => delivered,
                Err(error) => return Ok(json_error(StatusCode::CONFLICT, error)),
            }
        }
        "cancel" => match manager.cancel_user_prompt(&session_id, &body.request_id).await {
            Ok(delivered) => delivered,
            Err(error) => return Ok(json_error(StatusCode::CONFLICT, error)),
        },
        other => {
            return Ok(json_error(
                StatusCode::BAD_REQUEST,
                format!("Unknown user prompt action: {other}"),
            ))
        }
    };

    Ok(json_ok(&serde_json::json!({ "delivered": delivered })))
}

async fn handle_tool_approval(
    session_ref: String,
    manager: Arc<AgentSessionManager>,
    body: ToolApprovalBody,
) -> Result<JsonReply, Infallible> {
    let session_id = match resolve_http_session_ref(&session_ref).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(json_error(status, error)),
    };
    match respond_tool_approval(manager.as_ref(), &session_id, &body.tool_call_id, body.approved)
        .await
    {
        Ok(()) => Ok(json_ok(&serde_json::json!({ "ok": true }))),
        Err(error) => {
            let status = if error.contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::BAD_REQUEST
            };
            Ok(json_error(status, error))
        }
    }
}

/// Routes for the E2E control plane. The caller (routes wiring) additionally
/// wraps these with the loopback Host guard and the loopback peer guard.
pub fn e2e_control_routes(
    agent_manager: impl Filter<Extract = (Arc<AgentSessionManager>,), Error = Infallible>
        + Clone
        + Send,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let capability: Option<String> = capability_token().map(str::to_string);
    let guarded = capability_guard(capability);

    // GET /api/e2e-control/status
    let status = warp::get()
        .and(warp::path("api"))
        .and(warp::path("e2e-control"))
        .and(warp::path("status"))
        .and(warp::path::end())
        .and(guarded.clone())
        .and_then(handle_status);

    // GET /api/e2e-control/sessions/:ref/interactions
    let interactions = warp::get()
        .and(warp::path("api"))
        .and(warp::path("e2e-control"))
        .and(warp::path("sessions"))
        .and(warp::path::param::<String>())
        .and(warp::path("interactions"))
        .and(warp::path::end())
        .and(guarded.clone())
        .and(agent_manager.clone())
        .and_then(handle_get_interactions);

    // POST /api/e2e-control/sessions/:ref/user-prompts
    let user_prompt = warp::post()
        .and(warp::path("api"))
        .and(warp::path("e2e-control"))
        .and(warp::path("sessions"))
        .and(warp::path::param::<String>())
        .and(warp::path("user-prompts"))
        .and(warp::path::end())
        .and(guarded.clone())
        .and(agent_manager.clone())
        .and(super::super::routes::json_body())
        .and_then(handle_user_prompt);

    // POST /api/e2e-control/sessions/:ref/tool-approvals
    let tool_approval = warp::post()
        .and(warp::path("api"))
        .and(warp::path("e2e-control"))
        .and(warp::path("sessions"))
        .and(warp::path::param::<String>())
        .and(warp::path("tool-approvals"))
        .and(warp::path::end())
        .and(guarded)
        .and(agent_manager)
        .and(super::super::routes::json_body())
        .and_then(handle_tool_approval);

    status.or(interactions).or(user_prompt).or(tool_approval)
}
