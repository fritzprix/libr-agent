//! UI automation routes for the E2E control plane (#2902, debug builds only).
//!
//! The interaction plane (approvals / prompts / circuit break / picker) only
//! answers blocking dialogs. These routes let an **external** tester drive and
//! observe the real GUI: screenshot the instance's own window, navigate, list
//! interactable elements, deliver window-scoped input, restart the instance,
//! and wait for a UI state. The tester (a multimodal agent) makes every
//! pass/fail judgement from the screenshots; nothing here contains verdict
//! logic.
//!
//! Window scoping comes from the process boundary itself: the app captures
//! **its own** windows (filtered by its own PID — never by title), input
//! coordinates are computed only from this window's inner origin plus
//! viewport-relative element rects and passed through the PID-scoped path of
//! `desktop__computerControl`, and element inspection runs inside this
//! webview. The user's other windows are structurally unreachable.
//!
//! The frontend does the DOM work and answers through
//! `crate::e2e_ui_bridge` (request-id report store + dialog counter); the
//! commands there are inert unless the control plane is enabled.
//!
//! Routes (all behind the shared `capability_guard`):
//!
//! - `GET  /api/e2e-control/ui/screenshot` — capture this PID's window to
//!   `<data_dir>/e2e_captures/`, optionally with base64.
//! - `POST /api/e2e-control/ui/navigate` — `{path?, sessionId?, panel?}`
//!   router / session / panel navigation.
//! - `POST /api/e2e-control/ui/elements` — list interactable elements
//!   (role / label / test id / viewport bounds).
//! - `POST /api/e2e-control/ui/input` — click / move / hover / type / key /
//!   scroll, targeted by `selector` / `testId` or window-viewport coordinates.
//! - `POST /api/e2e-control/ui/restart` — restart the instance in place.
//! - `POST /api/e2e-control/ui/wait` — wait for a UI/session state with a
//!   timeout (no verdict, just the observed state).

use std::convert::Infallible;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{Emitter, Manager};
use warp::http::StatusCode;
use warp::{Filter, Rejection, Reply};

use crate::agent::session_manager::{approvals::list_pending_approvals_in, pending_user_prompt_in};
use crate::agent::AgentSessionManager;
use crate::e2e_ui_bridge::{dialog_open, take_report};
use crate::mcp::builtin::app_control::handlers::APP_CONTROL_EVENT;
pub use crate::mcp::builtin::desktop::handlers::{
    clamp_to_window_bounds as clamp_to_viewport, handle_computer_control,
    pick_window_by_pid as pick_own_window,
};
use crate::mcp::builtin::media::{encode_png_base64, encode_rgba_png};
use crate::mcp::types::MCPResult;
use crate::server::e2e_control::circuit_break::{pending_circuit_break, PendingCircuitBreak};
use crate::server::e2e_control::handlers::{json_error, json_ok, JsonReply};
use crate::server::e2e_control::token::{explicit_data_dir, E2E_CONTROL_CAPABILITY_FILE};
use crate::server::handlers::helpers::resolve_http_session_ref;
use crate::state::get_app_handle;

/// The main app window label (default label from `tauri.conf.json`).
const WINDOW_LABEL: &str = "main";

/// How long to wait for the frontend bridge to answer one request.
const BRIDGE_DEFAULT_TIMEOUT_MS: u64 = 5000;
const BRIDGE_MAX_TIMEOUT_MS: u64 = 15000;

fn clamp_ms(value: Option<u64>, default: u64, min: u64, max: u64) -> u64 {
    value.unwrap_or(default).clamp(min, max)
}

// ---------------------------------------------------------------------------
// Pure decisions (unit-tested; the handlers only add I/O around them)
// ---------------------------------------------------------------------------

/// A rectangle in webview-viewport coordinates (what `getBoundingClientRect`
/// reports), as answered by the frontend bridge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewportRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Center of `rect` in absolute screen coordinates, given the webview's
/// absolute inner origin.
pub fn element_center(origin: (i32, i32), rect: &ViewportRect) -> (i32, i32) {
    (
        origin.0 + rect.x + rect.width as i32 / 2,
        origin.1 + rect.y + rect.height as i32 / 2,
    )
}

/// Validated window inner origin `(x, y)`, inner size `(width, height)`, and
/// DPI scale factor.
pub type ViewportGeometry = ((i32, i32), (u32, u32), f64);

/// Validate the window's inner origin, inner size, and scale factor before
/// delivering any input (#2902). Never falls back to `(0, 0)` on a failed
/// read-back, which could send input outside the instance window.
pub fn window_viewport_geometry<E: std::fmt::Display>(
    position: Result<(i32, i32), E>,
    size: Result<(u32, u32), E>,
    scale: Result<f64, E>,
) -> Result<ViewportGeometry, String> {
    let origin = position.map_err(|error| format!("inner_position failed: {error}"))?;
    let (width, height) = size.map_err(|error| format!("inner_size failed: {error}"))?;
    if width == 0 || height == 0 {
        return Err(format!(
            "inner_size failed: zero viewport size {width}x{height}"
        ));
    }
    let scale = scale.map_err(|error| format!("scale_factor failed: {error}"))?;
    if !(scale.is_finite() && scale > 0.0) {
        return Err(format!("scale_factor failed: invalid scale {scale}"));
    }
    Ok((origin, (width, height), scale))
}

/// Validate a `ui/navigate` path: same-origin router path only.
pub fn plan_navigate_path(raw: &str) -> Result<String, (StatusCode, String)> {
    let path = raw.trim();
    if path.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "path must not be empty".to_string(),
        ));
    }
    if !path.starts_with('/') || path.starts_with("//") {
        return Err((
            StatusCode::BAD_REQUEST,
            "path must be an absolute router path starting with a single '/'".to_string(),
        ));
    }
    if path.chars().any(|c| c.is_control() || c == ' ') {
        return Err((
            StatusCode::BAD_REQUEST,
            "path must not contain spaces or control characters".to_string(),
        ));
    }
    Ok(path.to_string())
}

/// Validate a `ui/navigate` panel name (`workspace`, `processes`, `planning`, or `close`).
pub fn plan_panel_name(raw: &str) -> Result<String, (StatusCode, String)> {
    let panel = raw.trim().to_ascii_lowercase();
    match panel.as_str() {
        "workspace" | "processes" | "planning" | "close" => Ok(panel),
        _ => Err((
            StatusCode::BAD_REQUEST,
            format!("unknown panel '{raw}'; supported: workspace, processes, planning, close"),
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NavigateBody {
    pub path: Option<String>,
    pub session_id: Option<String>,
    pub panel: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NavigatePlan {
    pub path: Option<String>,
    pub session_ref: Option<String>,
    pub panel: Option<String>,
}

/// Validate a `ui/navigate` body: at least one of `path`, `sessionId`, or
/// `panel` must be given, and `path` and `sessionId` are mutually exclusive.
pub fn plan_navigate(body: &NavigateBody) -> Result<NavigatePlan, (StatusCode, String)> {
    let path = match body.path.as_deref() {
        Some(raw) => Some(plan_navigate_path(raw)?),
        None => None,
    };
    let session_ref = body
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if path.is_some() && session_ref.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            "provide either path or sessionId, not both".to_string(),
        ));
    }
    let panel = match body.panel.as_deref() {
        Some(raw) => Some(plan_panel_name(raw)?),
        None => None,
    };
    if path.is_none() && session_ref.is_none() && panel.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "provide path, sessionId, and/or panel".to_string(),
        ));
    }
    Ok(NavigatePlan {
        path,
        session_ref,
        panel,
    })
}

/// Where an input action should land.
#[derive(Debug, Clone, PartialEq)]
pub enum InputTarget {
    /// Resolve an element through the frontend bridge.
    Element {
        selector: Option<String>,
        test_id: Option<String>,
    },
    /// Window-viewport coordinates (fallback targeting per the issue).
    Viewport { x: i32, y: i32 },
    /// The current cursor position — only allowed for actions that do not
    /// need a coordinate (`type`, `key`).
    Cursor,
}

/// A validated `ui/input` request, ready to be turned into `desktop` tool
/// arguments.
#[derive(Debug, PartialEq)]
pub struct InputPlan {
    pub action: String,
    pub target: InputTarget,
    pub text: Option<String>,
    pub key: Option<String>,
    pub modifiers: Vec<String>,
    pub scroll_amount: Option<i32>,
    pub axis: Option<String>,
    pub button: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputBody {
    pub action: String,
    pub selector: Option<String>,
    pub test_id: Option<String>,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub text: Option<String>,
    pub key: Option<String>,
    #[serde(default)]
    pub modifiers: Vec<String>,
    pub scroll_amount: Option<i32>,
    pub axis: Option<String>,
    pub button: Option<String>,
}

/// Actions that drive the pointer and therefore need a target.
const POINTER_ACTIONS: [&str; 6] = [
    "click",
    "double_click",
    "right_click",
    "middle_click",
    "move",
    "drag",
];
/// Actions that need no coordinate at all.
const NO_TARGET_ACTIONS: [&str; 2] = ["type", "key"];

/// Validate and normalize a `ui/input` body. `hover` maps to `move` (the
/// desktop handler has no separate hover). Pointer actions must target an
/// element or explicit window coordinates — never the free cursor — so input
/// always lands inside this instance's window.
pub fn plan_input(body: &InputBody) -> Result<InputPlan, (StatusCode, String)> {
    let action = match body.action.trim().to_ascii_lowercase().as_str() {
        "hover" => "move".to_string(),
        other => other.to_string(),
    };
    let known = POINTER_ACTIONS.contains(&action.as_str())
        || NO_TARGET_ACTIONS.contains(&action.as_str())
        || action == "scroll";
    if !known {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "unknown action '{action}'; supported: click, double_click, right_click, middle_click, move, hover, type, key, scroll"
            ),
        ));
    }

    let has_element = body
        .selector
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty())
        || body
            .test_id
            .as_deref()
            .is_some_and(|t| !t.trim().is_empty());
    let has_viewport = body.x.is_some() && body.y.is_some();
    if (body.x.is_some() ^ body.y.is_some()) && !has_element {
        return Err((
            StatusCode::BAD_REQUEST,
            "x and y must be provided together".to_string(),
        ));
    }

    let target = if has_element {
        InputTarget::Element {
            selector: body
                .selector
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            test_id: body
                .test_id
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string),
        }
    } else if has_viewport {
        InputTarget::Viewport {
            x: body.x.unwrap(),
            y: body.y.unwrap(),
        }
    } else if POINTER_ACTIONS.contains(&action.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "action '{action}' needs a target: selector, testId, or x/y window coordinates"
            ),
        ));
    } else {
        InputTarget::Cursor
    };

    if action == "type" && body.text.as_deref().unwrap_or("").is_empty() {
        return Err((StatusCode::BAD_REQUEST, "type requires text".to_string()));
    }
    if action == "key" && body.key.as_deref().unwrap_or("").is_empty() {
        return Err((StatusCode::BAD_REQUEST, "key requires key".to_string()));
    }
    if action == "scroll" && body.scroll_amount.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "scroll requires scrollAmount".to_string(),
        ));
    }

    Ok(InputPlan {
        action,
        target,
        text: body.text.clone(),
        key: body.key.clone(),
        modifiers: body.modifiers.clone(),
        scroll_amount: body.scroll_amount,
        axis: body.axis.clone(),
        button: body.button.clone(),
    })
}

/// Build the argument object for the reused `desktop__computerControl`
/// handler.
pub fn desktop_args(plan: &InputPlan, absolute: Option<(i32, i32)>) -> Value {
    let mut args = json!({ "action": &plan.action });
    if let Some((x, y)) = absolute {
        args["x"] = json!(x);
        args["y"] = json!(y);
    }
    if let Some(text) = &plan.text {
        args["text"] = json!(text);
    }
    if let Some(key) = &plan.key {
        args["key"] = json!(key);
    }
    if !plan.modifiers.is_empty() {
        args["modifiers"] = json!(&plan.modifiers);
    }
    if let Some(amount) = plan.scroll_amount {
        args["scroll_amount"] = json!(amount);
    }
    if let Some(axis) = &plan.axis {
        args["axis"] = json!(axis);
    }
    if let Some(button) = &plan.button {
        args["button"] = json!(button);
    }
    args
}

/// A UI/session state the harness can wait on. Each condition is an
/// observation, never a verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitCondition {
    SessionIdle,
    PromptPending,
    ApprovalPending,
    CircuitBreakPending,
    DialogOpen,
}

pub fn parse_wait_condition(raw: &str) -> Result<WaitCondition, (StatusCode, String)> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "session-idle" => Ok(WaitCondition::SessionIdle),
        "prompt-pending" => Ok(WaitCondition::PromptPending),
        "approval-pending" => Ok(WaitCondition::ApprovalPending),
        "circuit-break-pending" => Ok(WaitCondition::CircuitBreakPending),
        "dialog-open" => Ok(WaitCondition::DialogOpen),
        other => Err((
            StatusCode::BAD_REQUEST,
            format!(
                "unknown condition '{other}'; supported: session-idle, prompt-pending, approval-pending, circuit-break-pending, dialog-open"
            ),
        )),
    }
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

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScreenshotQuery {
    #[serde(default)]
    include_base64: bool,
}

/// Capture only this process's own window: enumerate windows, keep the ones
/// whose PID is ours, encode the first visible one to PNG using the shared
/// `media::handlers` PNG encoder.
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
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            format!("no '{WINDOW_LABEL}' webview window (headless)"),
        ));
    };
    // Window scoping: every coordinate below is derived from this window's
    // inner origin and clamped into it, so input can never land elsewhere.
    let _ = window.set_focus();
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

    let mut target_request_id: Option<String> = None;
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
        InputTarget::Element { selector, test_id } => {
            let request_id = uuid::Uuid::new_v4().to_string();
            if app
                .emit(
                    "e2e-target",
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
                        "frontend bridge did not answer the target request within {} ms",
                        timeout.as_millis()
                    ),
                ));
            };
            if report.get("found").and_then(Value::as_bool) != Some(true) {
                return Ok(json_error(
                    StatusCode::NOT_FOUND,
                    format!(
                        "no element matched selector={:?} testId={:?}",
                        selector.as_deref(),
                        test_id.as_deref()
                    ),
                ));
            }
            let rect: ViewportRect = match report.get("rect").cloned().map(serde_json::from_value) {
                Some(Ok(rect)) => rect,
                _ => {
                    return Ok(json_error(
                        StatusCode::BAD_GATEWAY,
                        "frontend bridge returned a malformed rect",
                    ))
                }
            };
            target_request_id = Some(request_id);
            let scaled_rect = ViewportRect {
                x: (rect.x as f64 * scale).round() as i32,
                y: (rect.y as f64 * scale).round() as i32,
                width: ((rect.width as f64 * scale).round() as u32).max(1),
                height: ((rect.height as f64 * scale).round() as u32).max(1),
            };
            Some(clamp_to_viewport(
                origin,
                size,
                element_center(origin, &scaled_rect),
            ))
        }
    };

    let own_pid = std::process::id();
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
    let mut args = desktop_args(&plan, absolute);
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
    let _ = app.emit(
        "e2e-after-input",
        json!({
            "requestId": target_request_id,
            "action": &plan.action,
            "text": &plan.text,
            "key": &plan.key,
            "modifiers": &plan.modifiers,
            "scrollAmount": &plan.scroll_amount,
            "axis": &plan.axis,
        }),
    );
    Ok(json_ok(&json!({ "ok": true, "result": text })))
}

/// Restart this instance in place: verify a running Tauri app exists first,
/// delete the create-exclusive capability file so the child can mint a fresh
/// token, then let Tauri re-exec the process.
async fn handle_restart() -> Result<JsonReply, Infallible> {
    let Some(app) = get_app_handle() else {
        return Ok(json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no app handle: cannot restart a headless process",
        ));
    };
    if let Some(data_dir) = explicit_data_dir() {
        let cap = data_dir.join(E2E_CONTROL_CAPABILITY_FILE);
        let _ = std::fs::remove_file(&cap);
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Let this response flush before the process goes away.
        tokio::time::sleep(Duration::from_millis(300)).await;
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

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_own_window_prefers_visible_and_ignores_other_pids() {
        let windows = [
            (10, 999, false),
            (20, 7, true),
            (30, 7, false),
            (40, 7, false),
        ];
        assert_eq!(pick_own_window(&windows, 7), Some(30));
        assert_eq!(pick_own_window(&windows, 999), Some(10));
        assert_eq!(pick_own_window(&windows, 1), None);
        // Only a minimized window left for our pid: still ours.
        let minimized = [(20, 7, true)];
        assert_eq!(pick_own_window(&minimized, 7), Some(20));
    }

    #[test]
    fn element_center_maps_viewport_rect_to_screen() {
        let rect = ViewportRect {
            x: 100,
            y: 50,
            width: 100,
            height: 40,
        };
        assert_eq!(element_center((200, 100), &rect), (350, 170));
    }

    #[test]
    fn clamp_to_viewport_keeps_input_inside_the_window() {
        let origin = (200, 100);
        let size = (800, 600);
        assert_eq!(clamp_to_viewport(origin, size, (500, 300)), (500, 300));
        assert_eq!(clamp_to_viewport(origin, size, (-5, -5)), (200, 100));
        assert_eq!(
            clamp_to_viewport(origin, size, (9999, 9999)),
            (200 + 799, 100 + 599)
        );
    }

    #[test]
    fn window_viewport_geometry_names_failing_read_backs() {
        assert_eq!(
            window_viewport_geometry::<&str>(Ok((10, 20)), Ok((800, 600)), Ok(1.5)).unwrap(),
            ((10, 20), (800, 600), 1.5)
        );
        assert!(
            window_viewport_geometry(Err("pos err"), Ok((800, 600)), Ok(1.0))
                .unwrap_err()
                .contains("inner_position failed")
        );
        assert!(
            window_viewport_geometry(Ok((0, 0)), Err("size err"), Ok(1.0))
                .unwrap_err()
                .contains("inner_size failed")
        );
        assert!(
            window_viewport_geometry::<&str>(Ok((0, 0)), Ok((0, 600)), Ok(1.0))
                .unwrap_err()
                .contains("zero viewport size")
        );
        assert!(
            window_viewport_geometry::<&str>(Ok((0, 0)), Ok((800, 600)), Ok(0.0))
                .unwrap_err()
                .contains("scale_factor failed")
        );
    }

    #[test]
    fn navigate_path_rejects_absolute_urls_and_garbage() {
        assert_eq!(plan_navigate_path("/chat").unwrap(), "/chat");
        assert_eq!(plan_navigate_path("  /settings  ").unwrap(), "/settings");
        assert!(plan_navigate_path("").is_err());
        assert!(plan_navigate_path("chat").is_err());
        assert!(plan_navigate_path("//evil.example.com").is_err());
        assert!(plan_navigate_path("https://evil.example.com/").is_err());
        assert!(plan_navigate_path("/has space").is_err());
    }

    #[test]
    fn plan_navigate_accepts_path_session_or_panel_and_rejects_invalid_combos() {
        assert_eq!(
            plan_navigate(&NavigateBody {
                path: Some("/settings".to_string()),
                session_id: None,
                panel: None,
            })
            .unwrap(),
            NavigatePlan {
                path: Some("/settings".to_string()),
                session_ref: None,
                panel: None,
            }
        );
        assert_eq!(
            plan_navigate(&NavigateBody {
                path: None,
                session_id: Some("sess-1".to_string()),
                panel: Some("Planning".to_string()),
            })
            .unwrap(),
            NavigatePlan {
                path: None,
                session_ref: Some("sess-1".to_string()),
                panel: Some("planning".to_string()),
            }
        );
        assert!(plan_navigate(&NavigateBody {
            path: None,
            session_id: None,
            panel: None,
        })
        .is_err());
        assert!(plan_navigate(&NavigateBody {
            path: Some("/agent".to_string()),
            session_id: Some("sess-1".to_string()),
            panel: None,
        })
        .is_err());
        assert!(plan_navigate(&NavigateBody {
            path: None,
            session_id: None,
            panel: Some("nonexistent".to_string()),
        })
        .is_err());
    }

    fn body(action: &str) -> InputBody {
        InputBody {
            action: action.to_string(),
            selector: None,
            test_id: None,
            x: None,
            y: None,
            text: None,
            key: None,
            modifiers: vec![],
            scroll_amount: None,
            axis: None,
            button: None,
        }
    }

    #[test]
    fn plan_input_requires_a_target_for_pointer_actions() {
        let error = plan_input(&body("click")).unwrap_err();
        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert!(error.1.contains("needs a target"));
    }

    #[test]
    fn plan_input_maps_hover_to_move_and_accepts_coordinates() {
        let mut b = body("hover");
        b.x = Some(10);
        b.y = Some(20);
        let plan = plan_input(&b).unwrap();
        assert_eq!(plan.action, "move");
        assert_eq!(plan.target, InputTarget::Viewport { x: 10, y: 20 });
    }

    #[test]
    fn plan_input_element_target_beats_viewport() {
        let mut b = body("click");
        b.selector = Some("button[role=tab]".to_string());
        b.x = Some(1);
        b.y = Some(2);
        let plan = plan_input(&b).unwrap();
        assert_eq!(
            plan.target,
            InputTarget::Element {
                selector: Some("button[role=tab]".to_string()),
                test_id: None
            }
        );
    }

    #[test]
    fn plan_input_validates_action_payloads() {
        assert!(plan_input(&body("type")).unwrap_err().1.contains("text"));
        assert!(plan_input(&body("key")).unwrap_err().1.contains("key"));
        assert!(plan_input(&body("scroll"))
            .unwrap_err()
            .1
            .contains("scrollAmount"));
        assert!(plan_input(&body("telekinesis")).is_err());
        let mut b = body("type");
        b.text = Some("dummy-token-abcd".to_string());
        assert_eq!(plan_input(&b).unwrap().target, InputTarget::Cursor);
    }

    #[test]
    fn desktop_args_carry_plan_fields() {
        let mut b = body("scroll");
        b.scroll_amount = Some(-3);
        b.axis = Some("vertical".to_string());
        let plan = plan_input(&b).unwrap();
        let args = desktop_args(&plan, Some((400, 300)));
        assert_eq!(args["action"], "scroll");
        assert_eq!(args["x"], 400);
        assert_eq!(args["scroll_amount"], -3);
        assert_eq!(args["axis"], "vertical");
        assert!(args.get("text").is_none());
    }

    #[test]
    fn wait_conditions_parse() {
        assert_eq!(
            parse_wait_condition("session-idle").unwrap(),
            WaitCondition::SessionIdle
        );
        assert_eq!(
            parse_wait_condition(" Dialog-Open ").unwrap(),
            WaitCondition::DialogOpen
        );
        assert!(parse_wait_condition("session-busy").is_err());
    }
}
