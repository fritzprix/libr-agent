//! E2E control plane handlers (debug builds only).
//!
//! Routes let an unattended harness observe and answer the two blocking
//! interactions a session can wait on:
//!
//! - the shared user prompt (#2543: `ui__askUser` / secret requests),
//! - pending tool-approval requests, and
//! - a waiting circuit-breaker Resume (#2901, `ui__resumeCircuitBreak` path —
//!   distinct from the Session API `POST /api/sessions/:id/resume` reload).
//!
//! Plus one dialog hook: `POST /picker` arms a one-shot override that
//! resolves the next native open/save dialog (#2903).
//!
//! Plus one window hook: `POST /window` shows/hides and resizes the main
//! window of the isolated debug instance (#2903 AC2).
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
use crate::agent::state::PendingUserPromptSummary;
use crate::agent::AgentSessionManager;
use crate::server::access_guard::AccessDenied;
use crate::server::e2e_control::circuit_break::{
    pending_circuit_break, resume_circuit_break_message, PendingCircuitBreak,
};
use crate::server::e2e_control::token::{capability_token, header_matches};
use crate::server::handlers::helpers::resolve_http_session_ref;

// Answering a user prompt is shared with the release Session API route
// (`POST /api/sessions/:id/channel/prompt`); re-exported so the debug plane
// and its tests keep importing from this module.
pub use crate::server::handlers::prompts::{
    plan_user_prompt_action, PlannedPromptAction, UserPromptBody,
};

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
    pub build_sha: &'static str,
    pub build_dirty: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInteractionsSnapshot {
    pub session_id: String,
    pub pending_user_prompt: Option<PendingUserPromptSummary>,
    pub pending_approvals: Vec<PendingApprovalSummary>,
    pub pending_circuit_break: Option<PendingCircuitBreak>,
}

/// The circuit break waiting for a Resume click, read from the session's
/// in-memory message cache. `None` for an unknown session or no waiting break.
async fn pending_circuit_break_in(
    manager: &AgentSessionManager,
    session_id: &str,
) -> Option<PendingCircuitBreak> {
    let messages = {
        let active = manager.active_sessions().read().await;
        active.get(session_id)?.messages.clone()
    };
    let messages = messages.read().await;
    pending_circuit_break(&messages)
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
        build_sha: crate::build_provenance::BUILD_SHA,
        build_dirty: crate::build_provenance::build_dirty(),
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
    if !manager
        .active_sessions()
        .read()
        .await
        .contains_key(&session_id)
    {
        return Ok(json_error(
            StatusCode::NOT_FOUND,
            format!("Session not found: {session_id}"),
        ));
    }
    let snapshot = SessionInteractionsSnapshot {
        session_id: session_id.clone(),
        pending_user_prompt: pending_user_prompt_in(manager.as_ref(), &session_id).await,
        pending_approvals: list_pending_approvals_in(manager.active_sessions(), &session_id).await,
        pending_circuit_break: pending_circuit_break_in(manager.as_ref(), &session_id).await,
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

    let pending = pending_user_prompt_in(manager.as_ref(), &session_id).await;
    let planned = match plan_user_prompt_action(pending.as_ref(), &body) {
        Ok(planned) => planned,
        Err((status, error)) => return Ok(json_error(status, error)),
    };

    let delivered = match planned {
        PlannedPromptAction::Answer(answer) => {
            match manager
                .answer_user_prompt(&session_id, &body.request_id, answer)
                .await
            {
                Ok(delivered) => delivered,
                Err(error) => return Ok(json_error(StatusCode::CONFLICT, error)),
            }
        }
        PlannedPromptAction::Cancel => {
            match manager
                .cancel_user_prompt(&session_id, &body.request_id)
                .await
            {
                Ok(delivered) => delivered,
                Err(error) => return Ok(json_error(StatusCode::CONFLICT, error)),
            }
        }
    };

    Ok(json_ok(&serde_json::json!({ "delivered": delivered })))
}

/// Body for `POST /api/e2e-control/picker`: exactly one of `path` / `cancel`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickerBody {
    pub path: Option<String>,
    pub cancel: Option<bool>,
}

/// Decide the picker override from the request body. Extracted as a pure
/// function so the validation is covered by tests.
pub fn plan_picker_override(
    body: &PickerBody,
) -> Result<crate::e2e_picker::PickerOverride, (StatusCode, String)> {
    let cancel = body.cancel.unwrap_or(false);
    match (
        body.path.as_deref().filter(|p| !p.trim().is_empty()),
        cancel,
    ) {
        (Some(path), false) => Ok(crate::e2e_picker::PickerOverride::Path {
            path: path.to_string(),
        }),
        (None, true) => Ok(crate::e2e_picker::PickerOverride::Cancel),
        (Some(_), true) => Err((
            StatusCode::BAD_REQUEST,
            "Provide either path or cancel, not both".to_string(),
        )),
        (None, false) => Err((
            StatusCode::BAD_REQUEST,
            "Provide path or cancel=true".to_string(),
        )),
    }
}

async fn handle_picker(body: PickerBody) -> Result<JsonReply, Infallible> {
    match plan_picker_override(&body) {
        Ok(override_) => {
            crate::e2e_picker::set_picker_override(override_);
            Ok(json_ok(&serde_json::json!({ "ok": true })))
        }
        Err((status, error)) => Ok(json_error(status, error)),
    }
}

/// Consume the currently armed one-shot picker override (same `take_picker_override`
/// path used by `download_commands::save_bytes_via_dialog` and the frontend wrapper).
async fn handle_picker_take() -> Result<JsonReply, Infallible> {
    let override_ = crate::e2e_picker::take_picker_override();
    Ok(json_ok(&serde_json::json!({ "override": override_ })))
}

/// Body for `POST /api/e2e-control/window` (#2903 AC2): show/hide and resize
/// of the isolated instance's main window. At least one field is required;
/// `width` / `height` are logical pixels (window size) and must be given
/// together.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowBody {
    pub visible: Option<bool>,
    pub width: Option<f64>,
    pub height: Option<f64>,
}

/// Upper bound for a requested window size in logical pixels. Larger values
/// are harness bugs; answering 400 beats an opaque OS-level failure.
const MAX_WINDOW_SIZE: f64 = 16_384.0;

/// Validation decision for a window request, kept pure so the guard-chain
/// tests can pin it without a live window.
#[derive(Debug, PartialEq)]
pub enum WindowPlan {
    Apply {
        visible: Option<bool>,
        width: Option<f64>,
        height: Option<f64>,
    },
    Reject(&'static str),
}

pub fn plan_window(body: &WindowBody) -> WindowPlan {
    if body.visible.is_none() && body.width.is_none() && body.height.is_none() {
        return WindowPlan::Reject("Provide visible, width+height, or both");
    }
    if body.width.is_some() != body.height.is_some() {
        return WindowPlan::Reject("Provide width and height together");
    }
    for size in [body.width, body.height].into_iter().flatten() {
        if !(size.is_finite() && size > 0.0 && size <= MAX_WINDOW_SIZE) {
            return WindowPlan::Reject("width and height must be in (0, 16384]");
        }
    }
    WindowPlan::Apply {
        visible: body.visible,
        width: body.width,
        height: body.height,
    }
}

/// Show/hide and resize the main window of the isolated instance (#2903).
/// Replies with the actual post-apply state — window managers clamp size
/// requests, so the read-back is what the harness should trust.
async fn handle_window(body: WindowBody) -> Result<JsonReply, Infallible> {
    use tauri::Manager;

    let (visible, width, height) = match plan_window(&body) {
        WindowPlan::Apply {
            visible,
            width,
            height,
        } => (visible, width, height),
        WindowPlan::Reject(message) => return Ok(json_error(StatusCode::BAD_REQUEST, message)),
    };
    let Some(app) = crate::state::get_app_handle() else {
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "No running Tauri app in this process",
        ));
    };
    let Some(window) = app.get_webview_window("main") else {
        return Ok(json_error(StatusCode::NOT_FOUND, "main window not found"));
    };
    if let Some(visible) = visible {
        let result = if visible {
            window.show()
        } else {
            window.hide()
        };
        if let Err(error) = result {
            return Ok(json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("show/hide failed: {error}"),
            ));
        }
    }
    if let (Some(width), Some(height)) = (width, height) {
        if let Err(error) = window.set_size(tauri::LogicalSize::new(width, height)) {
            return Ok(json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("set_size failed: {error}"),
            ));
        }
    }
    let reply = window_state_reply(
        window.is_visible(),
        window.outer_size().map(|size| (size.width, size.height)),
        window.scale_factor(),
    );
    Ok(match reply {
        Ok(state) => json_ok(&state),
        Err(message) => json_error(StatusCode::INTERNAL_SERVER_ERROR, message),
    })
}

/// Build the `200` reply of `POST /api/e2e-control/window` from the read-back
/// calls (#2903 G1.1). Any failed call yields an error naming it, so a `200`
/// always carries real values. `size` is the physical outer size in pixels;
/// the reply converts it to logical pixels with `scale`.
pub fn window_state_reply<E: std::fmt::Display>(
    visible: Result<bool, E>,
    size: Result<(u32, u32), E>,
    scale: Result<f64, E>,
) -> Result<serde_json::Value, String> {
    let visible = visible.map_err(|error| format!("is_visible failed: {error}"))?;
    let (width, height) = size.map_err(|error| format!("outer_size failed: {error}"))?;
    let scale = scale.map_err(|error| format!("scale_factor failed: {error}"))?;
    if !(scale.is_finite() && scale > 0.0) {
        return Err(format!("scale_factor failed: invalid scale {scale}"));
    }
    Ok(serde_json::json!({
        "visible": visible,
        "width": f64::from(width) / scale,
        "height": f64::from(height) / scale,
    }))
}

/// Map a tool-approval rejection message to the HTTP status the handler
/// answers with. Extracted so the status mapping is covered by tests.
pub fn tool_approval_error_status(message: &str) -> StatusCode {
    if message.contains("not found") {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::BAD_REQUEST
    }
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
    match respond_tool_approval(
        manager.as_ref(),
        &session_id,
        &body.tool_call_id,
        body.approved,
    )
    .await
    {
        Ok(()) => Ok(json_ok(&serde_json::json!({ "ok": true }))),
        Err(error) => Ok(json_error(tool_approval_error_status(&error), error)),
    }
}

/// Drive the circuit-breaker Resume path (#2901): inject the same assistant
/// `ui__resumeCircuitBreak` tool-call message the GUI Resume button sends.
/// Distinct from the Session API reload route `POST /api/sessions/:id/resume`
/// (#2898), which does not touch the circuit-breaker dialog.
async fn handle_circuit_break_resume(
    session_ref: String,
    manager: Arc<AgentSessionManager>,
) -> Result<JsonReply, Infallible> {
    let session_id = match resolve_http_session_ref(&session_ref).await {
        Ok(id) => id,
        Err((status, error)) => return Ok(json_error(status, error)),
    };
    let pending = pending_circuit_break_in(manager.as_ref(), &session_id).await;
    let (tool_name, repetition_count) = match plan_circuit_break_resume(pending) {
        Ok(plan) => plan,
        Err((status, error)) => return Ok(json_error(status, error)),
    };
    let message = resume_circuit_break_message(&session_id, &tool_name, repetition_count);
    match manager.handle_llm_response(session_id, message).await {
        Ok(()) => Ok(json_ok(&serde_json::json!({
            "ok": true,
            "resumedTool": tool_name,
            "repetitionCount": repetition_count,
        }))),
        Err(error) => Ok(json_error(StatusCode::CONFLICT, error)),
    }
}

/// The pure dispatch decision behind [`handle_circuit_break_resume`]: a
/// session with no waiting circuit breaker is rejected with 409, and a
/// waiting one yields the tool name + repetition count to inject. Tested
/// directly because a live `AgentSessionManager` needs a Tauri `AppHandle`
/// that integration tests cannot construct.
pub fn plan_circuit_break_resume(
    pending: Option<PendingCircuitBreak>,
) -> Result<(String, u64), (StatusCode, &'static str)> {
    match pending {
        Some(pending) => Ok((pending.tool_name, pending.repetition_count)),
        None => Err((
            StatusCode::CONFLICT,
            "no circuit breaker is waiting for resume on this session",
        )),
    }
}

/// Routes for the E2E control plane, using this process's capability token.
/// Production wiring goes through
/// [`e2e_control_guarded_routes`](super::e2e_control_guarded_routes), which
/// adds the loopback Host guard and the loopback peer guard.
pub fn e2e_control_routes(
    agent_manager: impl Filter<Extract = (Arc<AgentSessionManager>,), Error = Infallible> + Clone + Send,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    e2e_control_routes_with_capability(capability_token().map(str::to_string), agent_manager)
}

/// Same routes with the capability passed in explicitly, so tests exercise
/// the real route set (paths, guards, handlers) without touching the
/// process-global token.
pub fn e2e_control_routes_with_capability(
    capability: Option<String>,
    agent_manager: impl Filter<Extract = (Arc<AgentSessionManager>,), Error = Infallible> + Clone + Send,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
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
        .and(guarded.clone())
        .and(agent_manager.clone())
        .and(super::super::routes::json_body())
        .and_then(handle_tool_approval);

    // POST /api/e2e-control/sessions/:ref/circuit-break-resume
    let circuit_break_resume = warp::post()
        .and(warp::path("api"))
        .and(warp::path("e2e-control"))
        .and(warp::path("sessions"))
        .and(warp::path::param::<String>())
        .and(warp::path("circuit-break-resume"))
        .and(warp::path::end())
        .and(guarded.clone())
        .and(agent_manager)
        .and_then(handle_circuit_break_resume);

    // POST /api/e2e-control/picker (no agent manager: resolves the next native dialog)
    let picker = warp::post()
        .and(warp::path("api"))
        .and(warp::path("e2e-control"))
        .and(warp::path("picker"))
        .and(warp::path::end())
        .and(guarded.clone())
        .and(super::super::routes::json_body())
        .and_then(handle_picker);

    // POST /api/e2e-control/picker/take (consume one-shot override for verification/cleanup)
    let picker_take = warp::post()
        .and(warp::path("api"))
        .and(warp::path("e2e-control"))
        .and(warp::path("picker"))
        .and(warp::path("take"))
        .and(warp::path::end())
        .and(guarded.clone())
        .and_then(handle_picker_take);

    // POST /api/e2e-control/window (no agent manager: show/hide + resize the main window, #2903)
    let window = warp::post()
        .and(warp::path("api"))
        .and(warp::path("e2e-control"))
        .and(warp::path("window"))
        .and(warp::path::end())
        .and(guarded)
        .and(super::super::routes::json_body())
        .and_then(handle_window);

    status
        .or(interactions)
        .or(user_prompt)
        .or(tool_approval)
        .or(circuit_break_resume)
        .or(picker)
        .or(picker_take)
        .or(window)
}
