//! Pure planning, validation, and window-scoping decisions for the debug E2E
//! UI control plane (#2902).
//!
//! Kept free of Tauri/runtime state so every decision — PID selection,
//! focus + z-order occlusion checks, viewport coordinate mapping, route/panel
//! validation, input normalization, and wait-condition parsing — is directly
//! unit-testable.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use warp::http::StatusCode;

pub use crate::mcp::builtin::desktop::handlers::{
    clamp_to_window_bounds as clamp_to_viewport, pick_window_by_pid as pick_own_window,
};

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

/// One window in front-to-back z-order (as returned by `xcap::Window::all()`)
/// for occlusion checks before OS input delivery (#2902 G1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZOrderedWindowFact {
    pub id: u32,
    pub pid: u32,
    pub minimized: bool,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl ZOrderedWindowFact {
    pub fn contains_point(&self, (px, py): (i32, i32)) -> bool {
        if self.minimized || self.width == 0 || self.height == 0 {
            return false;
        }
        let max_x = self.x.saturating_add(self.width as i32);
        let max_y = self.y.saturating_add(self.height as i32);
        px >= self.x && px < max_x && py >= self.y && py < max_y
    }
}

/// Verify that OS input will land in `own_pid`'s window and nowhere else
/// (#2902 G1.1):
///
/// 1. The instance's Tauri window must be focused (`focused == true`). If
///    focus-stealing prevention or the compositor left another window focused,
///    refuse with `409 Conflict` instead of sending keystrokes or clicks to
///    the user's active window.
/// 2. When a target screen coordinate `target_point` is given, inspect the
///    z-ordered windows front-to-back (`xcap::Window::all()`). The first
///    non-minimized window covering `target_point` must belong to `own_pid`;
///    if a foreign PID's window sits above `target_point` (or no visible
///    window of `own_pid` covers it), refuse with `409 Conflict`.
pub fn verify_window_input_safety(
    focused: bool,
    z_ordered_windows: &[ZOrderedWindowFact],
    own_pid: u32,
    target_point: Option<(i32, i32)>,
) -> Result<(), (StatusCode, String)> {
    if !focused {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "window owned by pid {own_pid} is not focused; refusing to deliver OS input while another window holds focus"
            ),
        ));
    }
    if let Some(point) = target_point {
        let topmost = z_ordered_windows
            .iter()
            .find(|window| window.contains_point(point));
        match topmost {
            Some(window) if window.pid == own_pid => Ok(()),
            Some(window) => Err((
                StatusCode::CONFLICT,
                format!(
                    "target point ({}, {}) is occluded by topmost window {} owned by foreign pid {} (expected own pid {own_pid})",
                    point.0, point.1, window.id, window.pid
                ),
            )),
            None => Err((
                StatusCode::CONFLICT,
                format!(
                    "no visible window owned by pid {own_pid} covers target point ({}, {})",
                    point.0, point.1
                ),
            )),
        }
    } else {
        Ok(())
    }
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
    /// The focused element in this window — only allowed for actions that do
    /// not need a pointer coordinate (`type`, `key`).
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

/// Actions that drive the pointer and therefore need a target (#2902 G1.2:
/// `drag` is intentionally omitted because single-target `ui/input` does not
/// carry drag start/end pairs).
pub const POINTER_ACTIONS: [&str; 5] = [
    "click",
    "double_click",
    "right_click",
    "middle_click",
    "move",
];
/// Actions that need no pointer coordinate.
pub const NO_TARGET_ACTIONS: [&str; 2] = ["type", "key"];

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
        let minimized = [(20, 7, true)];
        assert_eq!(pick_own_window(&minimized, 7), Some(20));
    }

    #[test]
    fn verify_window_input_safety_checks_focus_and_z_order_occlusion() {
        let own_window = ZOrderedWindowFact {
            id: 10,
            pid: 42,
            minimized: false,
            x: 100,
            y: 100,
            width: 800,
            height: 600,
        };
        let foreign_overlay = ZOrderedWindowFact {
            id: 99,
            pid: 999,
            minimized: false,
            x: 200,
            y: 150,
            width: 400,
            height: 300,
        };
        let minimized_foreign = ZOrderedWindowFact {
            minimized: true,
            ..foreign_overlay
        };

        // 1. Own window focused and topmost at target point -> allowed.
        assert!(verify_window_input_safety(
            true,
            &[minimized_foreign, own_window, foreign_overlay],
            42,
            Some((300, 250)),
        )
        .is_ok());

        // 2. Foreign non-minimized window above the target point -> refused (409).
        let err =
            verify_window_input_safety(true, &[foreign_overlay, own_window], 42, Some((300, 250)))
                .unwrap_err();
        assert_eq!(err.0, StatusCode::CONFLICT);
        assert!(err.1.contains("occluded by topmost window 99"));
        assert!(err.1.contains("foreign pid 999"));

        // 3. Point outside the foreign overlay but still inside own window -> allowed.
        assert!(verify_window_input_safety(
            true,
            &[foreign_overlay, own_window],
            42,
            Some((150, 150)),
        )
        .is_ok());

        // 4. Unfocused window -> refused for type/key (no target point) and for pointer actions.
        let unfocused_key = verify_window_input_safety(false, &[own_window], 42, None).unwrap_err();
        assert_eq!(unfocused_key.0, StatusCode::CONFLICT);
        assert!(unfocused_key.1.contains("not focused"));

        let unfocused_click =
            verify_window_input_safety(false, &[own_window], 42, Some((300, 250))).unwrap_err();
        assert_eq!(unfocused_click.0, StatusCode::CONFLICT);
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
    fn plan_input_requires_a_target_for_pointer_actions_and_rejects_drag() {
        let error = plan_input(&body("click")).unwrap_err();
        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert!(error.1.contains("needs a target"));

        let mut drag = body("drag");
        drag.x = Some(10);
        drag.y = Some(20);
        let drag_err = plan_input(&drag).unwrap_err();
        assert_eq!(drag_err.0, StatusCode::BAD_REQUEST);
        assert!(drag_err.1.contains("unknown action 'drag'"));
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
