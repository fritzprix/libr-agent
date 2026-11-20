//! UI automation HTTP handlers and routes for the E2E control plane (#2902,
//! debug builds only).
//!
//! Pure planning and window-safety decisions live in [`super::ui_plan`].
//!
//! Input scope follows the owner ruling on #2902 G1.1: element targets
//! (selector / testId) are dispatched as DOM events inside this webview and
//! never touch OS input; only raw coordinates fall back to OS-level input,
//! gated by two layers before anything is sent:
//! 1. Target coordinates are derived from this window's inner origin +
//!    viewport-relative element rects and clamped into this window's viewport
//!    (`clamp_to_viewport` + `window_pid` in `desktop__computerControl`).
//! 2. [`verify_window_input_safety`] requires `window.is_focused()` to be
//!    true after `set_focus()` (refusing `type`/`key`/pointer input with
//!    `409 Conflict` if another window holds focus) and checks the z-ordered
//!    `xcap::Window::all()` list to ensure the topmost visible window at the
//!    target coordinate belongs to `std::process::id()` (refusing with
//!    `409 Conflict` if a foreign window overlaps the target point).

use std::convert::Infallible;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{Emitter, Manager};
use warp::http::StatusCode;
use warp::{Filter, Rejection, Reply};

use crate::agent::session_manager::{approvals::list_pending_approvals_in, pending_user_prompt_in};
use crate::agent::AgentSessionManager;
use crate::e2e_ui_bridge::{dialog_open, register_request, take_report};
use crate::mcp::builtin::app_control::handlers::APP_CONTROL_EVENT;
pub use crate::mcp::builtin::desktop::handlers::handle_computer_control;
use crate::mcp::builtin::media::{encode_png_base64, encode_rgba_png};
use crate::mcp::types::MCPResult;
use crate::server::e2e_control::circuit_break::{pending_circuit_break, PendingCircuitBreak};
use crate::server::e2e_control::handlers::{json_error, json_ok, JsonReply};
use crate::server::e2e_control::token::{explicit_data_dir, E2E_CONTROL_CAPABILITY_FILE};
use crate::server::handlers::helpers::resolve_http_session_ref;
use crate::state::get_app_handle;

pub use super::ui_plan::*;

/// The main app window label (default label from `tauri.conf.json`).
const WINDOW_LABEL: &str = "main";

/// How long to wait for the frontend bridge to answer one request.
const BRIDGE_DEFAULT_TIMEOUT_MS: u64 = 5000;
const BRIDGE_MAX_TIMEOUT_MS: u64 = 15000;

fn clamp_ms(value: Option<u64>, default: u64, min: u64, max: u64) -> u64 {
    value.unwrap_or(default).clamp(min, max)
}

/// Pull the first text part out of a `desktop` tool result.
fn mcp_text(result: &MCPResult) -> String {
    result
        .content
        .as_ref()
        .and_then(|parts| {
            parts.iter().find_map(|part| match part {
                crate::mcp::types::MCPContent::Text { text } => Some(text.clone()),
                _ => None,
            })
        })
        .unwrap_or_else(|| "no text result".to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScreenshotQuery {
    #[serde(default)]
    include_base64: bool,
}

/// Capture only this process's own window: enumerate windows, keep the ones
/// whose PID is ours, encode the first visible one to PNG using the shared
/// `media` PNG encoder.
fn capture_own_window(own_pid: u32) -> Result<(Vec<u8>, u32, u32), String> {
    let windows = xcap::Window::all().map_err(|e| format!("window enumeration failed: {e}"))?;
    let facts: Vec<(u32, u32, bool)> = windows
        .iter()
        .filter_map(|w| {
            let id = w.id().ok()?;
            let pid = w.pid().ok()?;
            let minimized = w.is_minimized().unwrap_or(false);
            Some((id, pid, minimized))
        })
        .collect();
    let id = pick_own_window(&facts, own_pid).ok_or_else(|| {
        format!("no window is owned by pid {own_pid} (headless process or window gone)")
    })?;
    let window = windows
        .iter()
        .find(|w| w.id().ok() == Some(id))
        .ok_or("window vanished mid-capture")?;
    let width = window
        .width()
        .map_err(|e| format!("window width failed: {e}"))?;
    let height = window
        .height()
        .map_err(|e| format!("window height failed: {e}"))?;
    let frame = window
        .capture_image()
        .map_err(|e| format!("window capture failed: {e}"))?;
    let png = encode_rgba_png(&frame)?;
    Ok((png, width, height))
}

/// Snapshot z-ordered windows (front-to-back) from `xcap::Window::all()` for
/// occlusion checks before pointer input (#2902 G1.1).
fn z_ordered_window_facts() -> Result<Vec<ZOrderedWindowFact>, String> {
    let windows = xcap::Window::all().map_err(|e| format!("window enumeration failed: {e}"))?;
    Ok(windows
        .iter()
        .filter_map(|w| {
            Some(ZOrderedWindowFact {
                id: w.id().ok()?,
                pid: w.pid().ok()?,
                minimized: w.is_minimized().unwrap_or(false),
                x: w.x().ok()?,
                y: w.y().ok()?,
                width: w.width().ok()?,
                height: w.height().ok()?,
            })
        })
        .collect())
}

async fn handle_screenshot(query: ScreenshotQuery) -> Result<JsonReply, Infallible> {
    let Some(data_dir) = explicit_data_dir() else {
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no explicit LIBRAGENT_DATA_DIR for capture output",
        ));
    };
    if get_app_handle().is_none() {
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no app handle: this process has no GUI window (headless)",
        ));
    }
    let own_pid = std::process::id();
    let captured = tokio::task::spawn_blocking(move || capture_own_window(own_pid)).await;
    let (png, width, height) = match captured {
        Ok(Ok(captured)) => captured,
        Ok(Err(error)) => {
            return Ok(json_error(StatusCode::SERVICE_UNAVAILABLE, error));
        }
        Err(error) => {
            return Ok(json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("capture task failed: {error}"),
            ));
        }
    };
    let dir = data_dir.join("e2e_captures");
    if let Err(error) = std::fs::create_dir_all(&dir) {
        return Ok(json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("cannot create {}: {error}", dir.display()),
        ));
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    let path = dir.join(format!("ui-{own_pid}-{stamp}.png"));
    if let Err(error) = std::fs::write(&path, &png) {
        return Ok(json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("cannot write {}: {error}", path.display()),
        ));
    }
    let mut body = json!({
        "path": path.to_string_lossy(),
        "width": width,
        "height": height,
        "bytes": png.len(),
    });
    if query.include_base64 {
        body["base64"] = json!(encode_png_base64(&png));
    }
    Ok(json_ok(&body))
}

async fn handle_navigate(body: NavigateBody) -> Result<JsonReply, Infallible> {
    let planned = match plan_navigate(&body) {
        Ok(planned) => planned,
        Err((status, error)) => return Ok(json_error(status, error)),
    };
    let resolved_path = match (planned.path, planned.session_ref) {
        (Some(path), _) => Some(path),
        (None, Some(session_ref)) => match resolve_http_session_ref(&session_ref).await {
            Ok(session_id) => Some(format!("/agent/{session_id}")),
            Err((status, error)) => return Ok(json_error(status, error)),
        },
        (None, None) => None,
    };
    let Some(app) = get_app_handle() else {
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no app handle: this process has no GUI window (headless)",
        ));
    };
    if let Some(ref path) = resolved_path {
        let _ = app.emit(APP_CONTROL_EVENT, json!({ "op": "navigate", "path": path }));
    }
    if app
        .emit(
            "e2e-navigate",
            json!({ "path": &resolved_path, "panel": &planned.panel }),
        )
        .is_err()
    {
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "could not reach the frontend webview",
        ));
    }
    Ok(json_ok(&json!({
        "ok": true,
        "path": resolved_path,
        "panel": planned.panel,
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElementsBody {
    pub selector: Option<String>,
    pub timeout_ms: Option<u64>,
}

/// Wait for the frontend bridge report for `request_id`, polling the shared
/// report store until the deadline.
async fn wait_for_report(request_id: &str, timeout: Duration) -> Option<Value> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(payload) = take_report(request_id) {
            return Some(payload);
        }
        if Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn handle_elements(body: ElementsBody) -> Result<JsonReply, Infallible> {
    let Some(app) = get_app_handle() else {
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no app handle: this process has no GUI window (headless)",
        ));
    };
    let request_id = uuid::Uuid::new_v4().to_string();
    register_request(&request_id);
    if app
        .emit(
            "e2e-inspect",
            json!({ "requestId": &request_id, "selector": &body.selector }),
        )
        .is_err()
    {
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "could not reach the frontend webview",
        ));
    }
    let timeout = Duration::from_millis(clamp_ms(
        body.timeout_ms,
        BRIDGE_DEFAULT_TIMEOUT_MS,
        100,
        BRIDGE_MAX_TIMEOUT_MS,
    ));
    let Some(report) = wait_for_report(&request_id, timeout).await else {
        return Ok(json_error(
            StatusCode::GATEWAY_TIMEOUT,
            format!(
                "frontend bridge did not answer within {} ms; is the GUI running with the control plane enabled?",
                timeout.as_millis()
            ),
        ));
    };
    let elements = report
        .get("elements")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(json_ok(
        &json!({ "elements": elements, "count": elements.len() }),
    ))
}

/// Deliver one `ui/input` action (#2902, owner ruling on the G1.1 input
/// scope):
///
/// - Element targets (selector / testId) run as **DOM events inside this
///   webview** through the frontend bridge — no OS-level input is sent for
///   them, so they can never reach another window on the desktop.
/// - Only raw viewport coordinates (and `type`/`key` aimed at the cursor)
///   fall back to OS-level input, and only after this instance's window is
///   verified focused (bounded re-check) and, for pointer targets, topmost
///   at the target point; otherwise `409 Conflict` is answered and nothing
///   is sent.
async fn handle_input(body: InputBody) -> Result<JsonReply, Infallible> {
    let plan = match plan_input(&body) {
        Ok(plan) => plan,
        Err((status, error)) => return Ok(json_error(status, error)),
    };
    let Some(app) = get_app_handle() else {
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no app handle: this process has no GUI window (headless)",
        ));
    };
    match &plan.target {
        InputTarget::Element { selector, test_id } => {
            handle_dom_input(app, &plan, selector.as_deref(), test_id.as_deref()).await
        }
        InputTarget::Viewport { .. } | InputTarget::Cursor => handle_os_input(app, &plan).await,
    }
}

/// Element-target input: resolve the element and dispatch the action as DOM
/// events inside the webview — the default path per the owner ruling.
async fn handle_dom_input(
    app: &tauri::AppHandle,
    plan: &InputPlan,
    selector: Option<&str>,
    test_id: Option<&str>,
) -> Result<JsonReply, Infallible> {
    let request_id = uuid::Uuid::new_v4().to_string();
    register_request(&request_id);
    if app
        .emit(
            "e2e-input",
            json!({
                "requestId": &request_id,
                "selector": selector,
                "testId": test_id,
                "action": &plan.action,
                "text": &plan.text,
                "key": &plan.key,
                "modifiers": &plan.modifiers,
                "scrollAmount": &plan.scroll_amount,
                "axis": &plan.axis,
                "button": &plan.button,
            }),
        )
        .is_err()
    {
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "could not reach the frontend webview",
        ));
    }
    let timeout = Duration::from_millis(BRIDGE_DEFAULT_TIMEOUT_MS);
    let Some(report) = wait_for_report(&request_id, timeout).await else {
        return Ok(json_error(
            StatusCode::GATEWAY_TIMEOUT,
            format!(
                "frontend bridge did not answer the input request within {} ms",
                timeout.as_millis()
            ),
        ));
    };
    if report.get("found").and_then(Value::as_bool) != Some(true) {
        return Ok(json_error(
            StatusCode::NOT_FOUND,
            format!("no element matched selector={selector:?} testId={test_id:?}"),
        ));
    }
    if report.get("dispatched").and_then(Value::as_bool) != Some(true) {
        return Ok(json_error(
            StatusCode::BAD_GATEWAY,
            format!("frontend bridge found the element but did not dispatch the action: {report}"),
        ));
    }
    Ok(json_ok(&json!({
        "ok": true,
        "delivery": "dom",
        "action": &plan.action,
        "description": report.get("description").cloned().unwrap_or(Value::Null),
        "rect": report.get("rect").cloned(),
    })))
}

/// Coordinate/cursor input: the OS-level fallback path, gated by the focus
/// and z-order topmost checks (#2902 G1.1) — input is refused, not
/// redirected, when this instance's window does not own the target.
async fn handle_os_input(
    app: &tauri::AppHandle,
    plan: &InputPlan,
) -> Result<JsonReply, Infallible> {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            format!("no '{WINDOW_LABEL}' webview window (headless)"),
        ));
    };
    if let Err(error) = window.set_focus() {
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            format!("set_focus failed: {error}"),
        ));
    }
    let mut focused = false;
    for _ in 0..10 {
        match window.is_focused() {
            Ok(true) => {
                focused = true;
                break;
            }
            Ok(false) => tokio::time::sleep(Duration::from_millis(25)).await,
            Err(error) => {
                return Ok(json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("is_focused failed: {error}"),
                ));
            }
        }
    }

    let (origin, size, scale) = match window_viewport_geometry(
        window.inner_position().map(|p| (p.x, p.y)),
        window.inner_size().map(|s| (s.width, s.height)),
        window.scale_factor(),
    ) {
        Ok(geometry) => geometry,
        Err(message) => {
            return Ok(json_error(StatusCode::INTERNAL_SERVER_ERROR, message));
        }
    };

    let absolute: Option<(i32, i32)> = match &plan.target {
        InputTarget::Cursor => None,
        InputTarget::Viewport { x, y } => {
            let scaled_x = (*x as f64 * scale).round() as i32;
            let scaled_y = (*y as f64 * scale).round() as i32;
            Some(clamp_to_viewport(
                origin,
                size,
                (origin.0 + scaled_x, origin.1 + scaled_y),
            ))
        }
        InputTarget::Element { .. } => {
            unreachable!("element targets are delivered through handle_dom_input")
        }
    };

    let own_pid = std::process::id();
    let z_windows = if absolute.is_some() {
        match tokio::task::spawn_blocking(z_ordered_window_facts).await {
            Ok(Ok(facts)) => facts,
            Ok(Err(error)) => {
                return Ok(json_error(StatusCode::SERVICE_UNAVAILABLE, error));
            }
            Err(error) => {
                return Ok(json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("window z-order task failed: {error}"),
                ));
            }
        }
    } else {
        Vec::new()
    };
    if let Err((status, error)) = verify_window_input_safety(focused, &z_windows, own_pid, absolute)
    {
        return Ok(json_error(status, error));
    }

    // Actions whose handler ignores coordinates still need the pointer
    // positioned first (scroll); click-family actions take x/y natively.
    if !POINTER_ACTIONS.contains(&plan.action.as_str()) {
        if let Some((x, y)) = absolute {
            let _ = handle_computer_control(json!({
                "action": "move",
                "x": x,
                "y": y,
                "window_pid": own_pid,
                "window_origin": [origin.0, origin.1],
                "window_size": [size.0, size.1],
            }))
            .await;
        }
    }
    let mut args = desktop_args(plan, absolute);
    args["window_pid"] = json!(own_pid);
    args["window_origin"] = json!([origin.0, origin.1]);
    args["window_size"] = json!([size.0, size.1]);

    let result = match handle_computer_control(args).await {
        Ok(result) => result,
        Err(error) => {
            return Ok(json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("desktop input handler failed: {error}"),
            ))
        }
    };
    let text = mcp_text(&result);
    if result.is_error == Some(true) {
        return Ok(json_error(StatusCode::BAD_GATEWAY, text));
    }
    Ok(json_ok(
        &json!({ "ok": true, "delivery": "os", "result": text }),
    ))
}

/// Restart this instance in place: verify a running Tauri app exists first,
/// then in the background task remove the create-exclusive capability file
/// immediately before `app.restart()` (#2902 S1.1).
async fn handle_restart() -> Result<JsonReply, Infallible> {
    let Some(app) = get_app_handle() else {
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no app handle: cannot restart a headless process",
        ));
    };
    let app = app.clone();
    let data_dir = explicit_data_dir();
    tauri::async_runtime::spawn(async move {
        // Let this response flush before the process goes away, and only
        // remove the capability file right as `app.restart()` executes.
        tokio::time::sleep(Duration::from_millis(300)).await;
        if let Some(dir) = data_dir {
            let _ = std::fs::remove_file(dir.join(E2E_CONTROL_CAPABILITY_FILE));
        }
        app.restart();
    });
    Ok(json_ok(&json!({
        "ok": true,
        "restarting": true,
        "note": "the process exits shortly; re-read <data_dir>/e2e_control.cap and the pid-keyed port file once the new instance is up",
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WaitBody {
    pub condition: String,
    pub session_id: Option<String>,
    pub timeout_ms: Option<u64>,
    pub poll_ms: Option<u64>,
}

/// The circuit break waiting for a Resume click, read from the session's
/// in-memory message cache (same source as the interactions snapshot).
async fn pending_break_in(
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

async fn handle_wait(
    body: WaitBody,
    manager: Arc<AgentSessionManager>,
) -> Result<JsonReply, Infallible> {
    let condition = match parse_wait_condition(&body.condition) {
        Ok(condition) => condition,
        Err((status, error)) => return Ok(json_error(status, error)),
    };
    let timeout = Duration::from_millis(clamp_ms(body.timeout_ms, 15_000, 0, 120_000));
    let poll = Duration::from_millis(clamp_ms(body.poll_ms, 250, 50, 2000));

    let session_id = if condition == WaitCondition::DialogOpen {
        None
    } else {
        let Some(raw) = body.session_id.as_deref().filter(|s| !s.trim().is_empty()) else {
            return Ok(json_error(
                StatusCode::BAD_REQUEST,
                format!("condition '{}' requires sessionId", body.condition),
            ));
        };
        match resolve_http_session_ref(raw).await {
            Ok(session_id) => Some(session_id),
            Err((status, error)) => return Ok(json_error(status, error)),
        }
    };

    let started = Instant::now();
    let (satisfied, state): (bool, Value) = loop {
        let (satisfied, state) = match condition {
            WaitCondition::SessionIdle => {
                let session_id = session_id.as_deref().unwrap_or_default();
                match manager.get_session(session_id).await {
                    Ok(Some(meta)) => (
                        meta.status == crate::repositories::SessionStatus::Idle,
                        json!({ "sessionStatus": meta.status.as_str() }),
                    ),
                    Ok(None) => (false, json!({ "sessionStatus": null })),
                    Err(error) => {
                        return Ok(json_error(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("session status read failed: {error}"),
                        ))
                    }
                }
            }
            WaitCondition::PromptPending => {
                let session_id = session_id.as_deref().unwrap_or_default();
                let pending = pending_user_prompt_in(manager.as_ref(), session_id).await;
                (pending.is_some(), json!({ "pendingUserPrompt": pending }))
            }
            WaitCondition::ApprovalPending => {
                let session_id = session_id.as_deref().unwrap_or_default();
                let pending =
                    list_pending_approvals_in(manager.active_sessions(), session_id).await;
                (!pending.is_empty(), json!({ "pendingApprovals": pending }))
            }
            WaitCondition::CircuitBreakPending => {
                let session_id = session_id.as_deref().unwrap_or_default();
                let pending = pending_break_in(manager.as_ref(), session_id).await;
                (pending.is_some(), json!({ "pendingCircuitBreak": pending }))
            }
            WaitCondition::DialogOpen => {
                let open = dialog_open();
                (open, json!({ "dialogOpen": open }))
            }
        };
        if satisfied || started.elapsed() >= timeout {
            break (satisfied, state);
        }
        tokio::time::sleep(poll).await;
    };

    Ok(json_ok(&json!({
        "condition": body.condition,
        "satisfied": satisfied,
        "timedOut": !satisfied,
        "waitedMs": started.elapsed().as_millis(),
        "state": state,
    })))
}

/// UI automation routes, guarded by [`super::handlers::capability_guard`].
pub fn ui_routes(
    capability: Option<String>,
    agent_manager: impl Filter<Extract = (Arc<AgentSessionManager>,), Error = Infallible> + Clone + Send,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let guarded = super::handlers::capability_guard(capability);
    // GET /api/e2e-control/ui/screenshot
    let screenshot = warp::path("api")
        .and(warp::path("e2e-control"))
        .and(warp::path("ui"))
        .and(warp::path("screenshot"))
        .and(warp::path::end())
        .and(guarded.clone())
        .and(warp::get())
        .and(warp::query::<ScreenshotQuery>())
        .and_then(handle_screenshot);

    // POST /api/e2e-control/ui/navigate
    let navigate = warp::path("api")
        .and(warp::path("e2e-control"))
        .and(warp::path("ui"))
        .and(warp::path("navigate"))
        .and(warp::path::end())
        .and(guarded.clone())
        .and(warp::post())
        .and(crate::server::routes::json_body())
        .and_then(handle_navigate);

    // POST /api/e2e-control/ui/elements
    let elements = warp::path("api")
        .and(warp::path("e2e-control"))
        .and(warp::path("ui"))
        .and(warp::path("elements"))
        .and(warp::path::end())
        .and(guarded.clone())
        .and(warp::post())
        .and(crate::server::routes::json_body())
        .and_then(handle_elements);

    // POST /api/e2e-control/ui/input
    let input = warp::path("api")
        .and(warp::path("e2e-control"))
        .and(warp::path("ui"))
        .and(warp::path("input"))
        .and(warp::path::end())
        .and(guarded.clone())
        .and(warp::post())
        .and(crate::server::routes::json_body())
        .and_then(handle_input);

    // POST /api/e2e-control/ui/restart
    let restart = warp::path("api")
        .and(warp::path("e2e-control"))
        .and(warp::path("ui"))
        .and(warp::path("restart"))
        .and(warp::path::end())
        .and(guarded.clone())
        .and(warp::post())
        .and_then(handle_restart);

    // POST /api/e2e-control/ui/wait
    let wait = warp::path("api")
        .and(warp::path("e2e-control"))
        .and(warp::path("ui"))
        .and(warp::path("wait"))
        .and(warp::path::end())
        .and(guarded)
        .and(warp::post())
        .and(crate::server::routes::json_body())
        .and(agent_manager)
        .and_then(handle_wait);

    screenshot
        .or(navigate)
        .or(elements)
        .or(input)
        .or(restart)
        .or(wait)
}
