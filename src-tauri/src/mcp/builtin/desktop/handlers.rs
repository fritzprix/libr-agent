use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use serde_json::Value;
use xcap::Monitor;

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::MCPResult;

/// Parse a string into an `enigo::Button`.
fn parse_button(s: &str) -> Result<Button, String> {
    match s.to_ascii_lowercase().as_str() {
        "left" => Ok(Button::Left),
        "right" => Ok(Button::Right),
        "middle" => Ok(Button::Middle),
        other => Err(format!(
            "Unsupported mouse button '{other}'. Allowed values: 'left', 'right', 'middle'."
        )),
    }
}

/// Parse a modifier key name into an `enigo::Key`.
fn parse_modifier(s: &str) -> Result<Key, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "ctrl" | "control" => Ok(Key::Control),
        "alt" | "opt" | "option" => Ok(Key::Alt),
        "shift" => Ok(Key::Shift),
        "meta" | "cmd" | "command" | "super" | "win" | "windows" => Ok(Key::Meta),
        other => Err(format!(
            "Unsupported modifier '{other}'. Allowed modifiers: 'ctrl', 'alt', 'shift', 'meta'."
        )),
    }
}

/// Parse a key name or single character into an `enigo::Key`.
fn parse_key(s: &str) -> Result<Key, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err("Key name cannot be empty.".to_string());
    }

    // Check known named keys
    match trimmed.to_ascii_lowercase().as_str() {
        "return" | "enter" => Ok(Key::Return),
        "tab" => Ok(Key::Tab),
        "space" => Ok(Key::Space),
        "backspace" => Ok(Key::Backspace),
        "escape" | "esc" => Ok(Key::Escape),
        "delete" | "del" => Ok(Key::Delete),
        "home" => Ok(Key::Home),
        "end" => Ok(Key::End),
        "pageup" | "page_up" => Ok(Key::PageUp),
        "pagedown" | "page_down" => Ok(Key::PageDown),
        "up" | "arrowup" | "arrow_up" => Ok(Key::UpArrow),
        "down" | "arrowdown" | "arrow_down" => Ok(Key::DownArrow),
        "left" | "arrowleft" | "arrow_left" => Ok(Key::LeftArrow),
        "right" | "arrowright" | "arrow_right" => Ok(Key::RightArrow),
        "capslock" | "caps_lock" => Ok(Key::CapsLock),
        "f1" => Ok(Key::F1),
        "f2" => Ok(Key::F2),
        "f3" => Ok(Key::F3),
        "f4" => Ok(Key::F4),
        "f5" => Ok(Key::F5),
        "f6" => Ok(Key::F6),
        "f7" => Ok(Key::F7),
        "f8" => Ok(Key::F8),
        "f9" => Ok(Key::F9),
        "f10" => Ok(Key::F10),
        "f11" => Ok(Key::F11),
        "f12" => Ok(Key::F12),
        "ctrl" | "control" => Ok(Key::Control),
        "alt" | "opt" | "option" => Ok(Key::Alt),
        "shift" => Ok(Key::Shift),
        "meta" | "cmd" | "command" | "super" | "win" | "windows" => Ok(Key::Meta),
        _ => {
            // Check if it's a single unicode char
            let mut chars = trimmed.chars();
            if let (Some(ch), None) = (chars.next(), chars.next()) {
                Ok(Key::Unicode(ch))
            } else {
                Err(format!(
                    "Unrecognized key name '{trimmed}'. For text entry, use action: 'type'. For special keys, use one of: 'Return', 'Enter', 'Tab', 'Space', 'Backspace', 'Escape', 'Delete', 'Home', 'End', 'PageUp', 'PageDown', 'Up', 'Down', 'Left', 'Right', 'CapsLock', 'F1'-'F12', or modifier names."
                ))
            }
        }
    }
}

/// Mapping used when converting image-local screenshot coordinates to absolute
/// virtual-desktop coordinates for OS input.
#[derive(Debug, Clone, Copy)]
struct ImageCoordMapping {
    origin_x: i32,
    origin_y: i32,
    width_scale: f64,
    height_scale: f64,
}

impl ImageCoordMapping {
    fn to_absolute(self, image_x: i32, image_y: i32) -> (i32, i32) {
        let abs_x = self.origin_x + (image_x as f64 * self.width_scale).round() as i32;
        let abs_y = self.origin_y + (image_y as f64 * self.height_scale).round() as i32;
        (abs_x, abs_y)
    }
}

/// Build an image→absolute mapping from `display_index` and/or explicit origin/scale
/// fields copied from `media__captureScreen` structured content.
fn resolve_image_coord_mapping(args: &Value) -> Result<Option<ImageCoordMapping>, String> {
    let display_index = match args.get("display_index") {
        None => None,
        Some(v) => {
            let Some(i) = v.as_i64() else {
                return Err("Parameter 'display_index' must be an integer.".to_string());
            };
            if i < 0 {
                return Err(format!(
                    "Parameter 'display_index' must be a non-negative integer, got {i}."
                ));
            }
            Some(i as usize)
        }
    };

    let origin_x = args.get("origin_x").and_then(|v| v.as_i64()).map(|v| v as i32);
    let origin_y = args.get("origin_y").and_then(|v| v.as_i64()).map(|v| v as i32);
    let width_scale = args.get("width_scale").and_then(|v| v.as_f64());
    let height_scale = args.get("height_scale").and_then(|v| v.as_f64());

    let has_origin = origin_x.is_some() || origin_y.is_some();
    if has_origin && (origin_x.is_none() || origin_y.is_none()) {
        return Err(
            "Both 'origin_x' and 'origin_y' must be provided together (copy from media__captureScreen)."
                .to_string(),
        );
    }
    if (width_scale.is_some() || height_scale.is_some())
        && (width_scale.is_none() || height_scale.is_none())
    {
        return Err(
            "Both 'width_scale' and 'height_scale' must be provided together (copy from media__captureScreen)."
                .to_string(),
        );
    }
    if let Some(ws) = width_scale {
        if !(ws.is_finite() && ws > 0.0) {
            return Err("Parameter 'width_scale' must be a finite number greater than 0.".to_string());
        }
    }
    if let Some(hs) = height_scale {
        if !(hs.is_finite() && hs > 0.0) {
            return Err(
                "Parameter 'height_scale' must be a finite number greater than 0.".to_string(),
            );
        }
    }

    if display_index.is_none() && origin_x.is_none() {
        // Absolute virtual-desktop coordinates (legacy / advanced mode).
        return Ok(None);
    }

    if let (Some(ox), Some(oy)) = (origin_x, origin_y) {
        return Ok(Some(ImageCoordMapping {
            origin_x: ox,
            origin_y: oy,
            width_scale: width_scale.unwrap_or(1.0),
            height_scale: height_scale.unwrap_or(1.0),
        }));
    }

    let idx = display_index.expect("display_index checked above");
    let monitors = Monitor::all().map_err(|e| format!("Failed to enumerate monitors: {e}"))?;
    if monitors.is_empty() {
        return Err("No active display monitors found on the system.".to_string());
    }
    let monitor = monitors.get(idx).ok_or_else(|| {
        format!(
            "Display index {idx} is out of range. Available displays: 0 to {}.",
            monitors.len().saturating_sub(1)
        )
    })?;

    let mon_x = monitor
        .x()
        .map_err(|e| format!("Failed to read monitor X origin: {e}"))?;
    let mon_y = monitor
        .y()
        .map_err(|e| format!("Failed to read monitor Y origin: {e}"))?;
    let scale = f64::from(monitor.scale_factor().unwrap_or(1.0));
    if !(scale.is_finite() && scale > 0.0) {
        return Err("Monitor scale_factor is invalid; cannot convert image coordinates.".to_string());
    }

    // Full-display captureScreen images are typically physical pixels while
    // monitor width/height are logical. Match captureScreen's width_scale =
    // monitor_w / image_w ≈ 1/scale_factor when image_w ≈ monitor_w * scale.
    let derived_width_scale = width_scale.unwrap_or(1.0 / scale);
    let derived_height_scale = height_scale.unwrap_or(1.0 / scale);

    Ok(Some(ImageCoordMapping {
        origin_x: mon_x,
        origin_y: mon_y,
        width_scale: derived_width_scale,
        height_scale: derived_height_scale,
    }))
}

fn map_optional_point(
    x: Option<i32>,
    y: Option<i32>,
    mapping: Option<ImageCoordMapping>,
) -> Result<(Option<i32>, Option<i32>), String> {
    match (x, y, mapping) {
        (None, None, _) => Ok((None, None)),
        (Some(_), None, _) | (None, Some(_), _) => Err(
            "Coordinates must provide both 'x' and 'y' or neither (to use current cursor position)."
                .to_string(),
        ),
        (Some(px), Some(py), None) => Ok((Some(px), Some(py))),
        (Some(px), Some(py), Some(m)) => {
            let (ax, ay) = m.to_absolute(px, py);
            Ok((Some(ax), Some(ay)))
        }
    }
}

/// Validate coordinates against physical monitor bounds if displays are accessible.
fn validate_coordinates(x: i32, y: i32) -> Result<(), String> {
    // Check against detected monitors
    match Monitor::all() {
        Ok(monitors) if !monitors.is_empty() => {
            let mut display_infos = Vec::new();
            let mut within_any = false;

            for (idx, mon) in monitors.iter().enumerate() {
                let mon_x = match mon.x() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let mon_y = match mon.y() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let mon_w = match mon.width() {
                    Ok(v) => v as i32,
                    Err(_) => continue,
                };
                let mon_h = match mon.height() {
                    Ok(v) => v as i32,
                    Err(_) => continue,
                };

                display_infos.push(format!(
                    "Display {idx}: x=[{mon_x}..{}], y=[{mon_y}..{}]",
                    mon_x + mon_w,
                    mon_y + mon_h
                ));

                if x >= mon_x && x < mon_x + mon_w && y >= mon_y && y < mon_y + mon_h {
                    within_any = true;
                    break;
                }
            }

            if !within_any {
                return Err(format!(
                    "Coordinates (x: {x}, y: {y}) are outside all detected screen boundaries. Available monitors: {}. Prefer media__captureScreen then desktop__computerControl with display_index + image-pixel x/y.",
                    display_infos.join("; ")
                ));
            }
        }
        Ok(_) | Err(_) => {
            // Fall back gracefully if display interrogation is restricted or headless
            log::debug!(
                "Could not enumerate monitors via xcap; skipping monitor bounds check for ({x}, {y})"
            );
        }
    }

    Ok(())
}

/// Dispatched handler for `desktop__computerControl`.
pub async fn handle_computer_control(args: Value) -> Result<MCPResult, String> {
    // 1. Extract action
    let action = match args.get("action").and_then(|v| v.as_str()) {
        Some(a) => a.trim().to_ascii_lowercase(),
        None => {
            return Ok(guided_error(
                ErrorCategory::MissingRequiredParam,
                "Missing required parameter 'action'.",
                ToolGroup::Desktop,
            )
            .to_mcp_result());
        }
    };

    // 2. Validate action and parameters
    let mapping = match resolve_image_coord_mapping(&args) {
        Ok(m) => m,
        Err(e) => {
            return Ok(guided_error(ErrorCategory::InvalidInput, e, ToolGroup::Desktop)
                .to_mcp_result());
        }
    };

    let raw_x = args.get("x").and_then(|v| v.as_i64()).map(|v| v as i32);
    let raw_y = args.get("y").and_then(|v| v.as_i64()).map(|v| v as i32);
    let raw_start_x = args.get("start_x").and_then(|v| v.as_i64()).map(|v| v as i32);
    let raw_start_y = args.get("start_y").and_then(|v| v.as_i64()).map(|v| v as i32);

    let (x, y) = match map_optional_point(raw_x, raw_y, mapping) {
        Ok(pair) => pair,
        Err(e) => {
            return Ok(guided_error(ErrorCategory::InvalidInput, e, ToolGroup::Desktop)
                .to_mcp_result());
        }
    };
    let (start_x, start_y) = match map_optional_point(raw_start_x, raw_start_y, mapping) {
        Ok(pair) => pair,
        Err(e) => {
            return Ok(guided_error(ErrorCategory::InvalidInput, e, ToolGroup::Desktop)
                .to_mcp_result());
        }
    };
    let button_str = args
        .get("button")
        .and_then(|v| v.as_str())
        .unwrap_or("left");
    let text = args.get("text").and_then(|v| v.as_str());
    let key_str = args.get("key").and_then(|v| v.as_str());
    let scroll_amount = args
        .get("scroll_amount")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);
    let axis_str = args
        .get("axis")
        .and_then(|v| v.as_str())
        .unwrap_or("vertical");

    // Extract modifiers
    let mut modifier_keys = Vec::new();
    if let Some(arr) = args.get("modifiers").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(mod_str) = item.as_str() {
                match parse_modifier(mod_str) {
                    Ok(k) => modifier_keys.push(k),
                    Err(e) => {
                        return Ok(guided_error(
                            ErrorCategory::InvalidInput,
                            e,
                            ToolGroup::Desktop,
                        )
                        .to_mcp_result());
                    }
                }
            }
        }
    }

    // Action validation
    match action.as_str() {
        "move" => {
            let (target_x, target_y) = match (x, y) {
                (Some(x_val), Some(y_val)) => (x_val, y_val),
                _ => {
                    return Ok(guided_error(
                        ErrorCategory::MissingRequiredParam,
                        "Action 'move' requires both 'x' and 'y' coordinates.",
                        ToolGroup::Desktop,
                    )
                    .to_mcp_result());
                }
            };
            if let Err(e) = validate_coordinates(target_x, target_y) {
                return Ok(guided_error(ErrorCategory::InvalidInput, e, ToolGroup::Desktop)
                    .to_mcp_result());
            }
        }
        "click" | "double_click" | "right_click" | "middle_click" | "mouse_down" | "mouse_up" => {
            if let (Some(target_x), Some(target_y)) = (x, y) {
                if let Err(e) = validate_coordinates(target_x, target_y) {
                    return Ok(guided_error(ErrorCategory::InvalidInput, e, ToolGroup::Desktop)
                        .to_mcp_result());
                }
            } else if (x.is_some() && y.is_none()) || (x.is_none() && y.is_some()) {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Coordinates must provide both 'x' and 'y' or neither (to use current cursor position).",
                    ToolGroup::Desktop,
                )
                .to_mcp_result());
            }
        }
        "drag" => {
            let (dest_x, dest_y) = match (x, y) {
                (Some(x_val), Some(y_val)) => (x_val, y_val),
                _ => {
                    return Ok(guided_error(
                        ErrorCategory::MissingRequiredParam,
                        "Action 'drag' requires target destination coordinates 'x' and 'y'.",
                        ToolGroup::Desktop,
                    )
                    .to_mcp_result());
                }
            };
            if let Err(e) = validate_coordinates(dest_x, dest_y) {
                return Ok(guided_error(ErrorCategory::InvalidInput, e, ToolGroup::Desktop)
                    .to_mcp_result());
            }
            if let (Some(s_x), Some(s_y)) = (start_x, start_y) {
                if let Err(e) = validate_coordinates(s_x, s_y) {
                    return Ok(guided_error(ErrorCategory::InvalidInput, e, ToolGroup::Desktop)
                        .to_mcp_result());
                }
            } else if (start_x.is_some() && start_y.is_none())
                || (start_x.is_none() && start_y.is_some())
            {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Both 'start_x' and 'start_y' must be provided if specifying drag start coordinates.",
                    ToolGroup::Desktop,
                )
                .to_mcp_result());
            }
        }
        "type" => {
            let text_val = match text {
                Some(t) => t,
                None => {
                    return Ok(guided_error(
                        ErrorCategory::MissingRequiredParam,
                        "Action 'type' requires parameter 'text'.",
                        ToolGroup::Desktop,
                    )
                    .to_mcp_result());
                }
            };
            if text_val.len() > 10_000 {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!(
                        "Text parameter exceeds maximum allowed length of 10,000 characters (got {}).",
                        text_val.len()
                    ),
                    ToolGroup::Desktop,
                )
                .to_mcp_result());
            }
            if text_val.contains('\0') {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Text string cannot contain null bytes.",
                    ToolGroup::Desktop,
                )
                .to_mcp_result());
            }
        }
        "key" => {
            if key_str.is_none() {
                return Ok(guided_error(
                    ErrorCategory::MissingRequiredParam,
                    "Action 'key' requires parameter 'key'.",
                    ToolGroup::Desktop,
                )
                .to_mcp_result());
            }
        }
        "scroll" => {
            if scroll_amount.is_none() {
                return Ok(guided_error(
                    ErrorCategory::MissingRequiredParam,
                    "Action 'scroll' requires parameter 'scroll_amount' (positive or negative integer).",
                    ToolGroup::Desktop,
                )
                .to_mcp_result());
            }
        }
        "cursor_position" => {
            // No additional parameters required
        }
        other => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Unknown action '{other}'. Supported actions: click, double_click, right_click, middle_click, move, mouse_down, mouse_up, drag, type, key, scroll, cursor_position."
                ),
                ToolGroup::Desktop,
            )
            .to_mcp_result());
        }
    }

    // 3. Execute OS input in spawn_blocking
    let action_clone = action.clone();
    let button_str = button_str.to_string();
    let text = text.map(|s| s.to_string());
    let key_str = key_str.map(|s| s.to_string());
    let axis_str = axis_str.to_string();

    let join_res = tokio::task::spawn_blocking(move || -> Result<(String, Option<serde_json::Value>), (ErrorCategory, String)> {
        let mut enigo = match Enigo::new(&Settings::default()) {
            Ok(e) => e,
            Err(enigo::NewConError::NoPermission) => {
                return Err((
                    ErrorCategory::PermissionDenied,
                    "Permission denied to simulate desktop input. Grant Accessibility permissions to LibrAgent in your OS settings.".to_string(),
                ));
            }
            Err(e) => {
                return Err((
                    ErrorCategory::OperationFailed,
                    format!("Failed to connect to display server / input simulation system: {e}"),
                ));
            }
        };

        match action_clone.as_str() {
            "move" => {
                let target_x = x.unwrap();
                let target_y = y.unwrap();
                enigo
                    .move_mouse(target_x, target_y, Coordinate::Abs)
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("Mouse move failed: {e}")))?;
                Ok((
                    format!("Successfully moved mouse cursor to (x: {target_x}, y: {target_y})"),
                    None,
                ))
            }
            "click" => {
                let btn = parse_button(&button_str)
                    .map_err(|e| (ErrorCategory::InvalidInput, e))?;
                if let (Some(tx), Some(ty)) = (x, y) {
                    enigo
                        .move_mouse(tx, ty, Coordinate::Abs)
                        .map_err(|e| (ErrorCategory::OperationFailed, format!("Mouse move before click failed: {e}")))?;
                }
                for modifier in &modifier_keys {
                    enigo
                        .key(*modifier, Direction::Press)
                        .map_err(|e| (ErrorCategory::OperationFailed, format!("Pressing modifier failed: {e}")))?;
                }
                enigo
                    .button(btn, Direction::Click)
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("Mouse click failed: {e}")))?;
                for modifier in modifier_keys.iter().rev() {
                    let _ = enigo.key(*modifier, Direction::Release);
                }
                let loc_str = match enigo.location() {
                    Ok((cur_x, cur_y)) => format!(" at (x: {cur_x}, y: {cur_y})"),
                    Err(_) => String::new(),
                };
                Ok((
                    format!("Successfully clicked '{button_str}' mouse button{loc_str}"),
                    None,
                ))
            }
            "double_click" => {
                let btn = parse_button(&button_str)
                    .map_err(|e| (ErrorCategory::InvalidInput, e))?;
                if let (Some(tx), Some(ty)) = (x, y) {
                    enigo
                        .move_mouse(tx, ty, Coordinate::Abs)
                        .map_err(|e| (ErrorCategory::OperationFailed, format!("Mouse move before double click failed: {e}")))?;
                }
                for modifier in &modifier_keys {
                    enigo
                        .key(*modifier, Direction::Press)
                        .map_err(|e| (ErrorCategory::OperationFailed, format!("Pressing modifier failed: {e}")))?;
                }
                enigo
                    .button(btn, Direction::Click)
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("First click of double click failed: {e}")))?;
                std::thread::sleep(std::time::Duration::from_millis(80));
                enigo
                    .button(btn, Direction::Click)
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("Second click of double click failed: {e}")))?;
                for modifier in modifier_keys.iter().rev() {
                    let _ = enigo.key(*modifier, Direction::Release);
                }
                let loc_str = match enigo.location() {
                    Ok((cur_x, cur_y)) => format!(" at (x: {cur_x}, y: {cur_y})"),
                    Err(_) => String::new(),
                };
                Ok((
                    format!("Successfully double clicked '{button_str}' mouse button{loc_str}"),
                    None,
                ))
            }
            "right_click" => {
                if let (Some(tx), Some(ty)) = (x, y) {
                    enigo
                        .move_mouse(tx, ty, Coordinate::Abs)
                        .map_err(|e| (ErrorCategory::OperationFailed, format!("Mouse move failed: {e}")))?;
                }
                enigo
                    .button(Button::Right, Direction::Click)
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("Right click failed: {e}")))?;
                let loc_str = match enigo.location() {
                    Ok((cur_x, cur_y)) => format!(" at (x: {cur_x}, y: {cur_y})"),
                    Err(_) => String::new(),
                };
                Ok((format!("Successfully performed right click{loc_str}"), None))
            }
            "middle_click" => {
                if let (Some(tx), Some(ty)) = (x, y) {
                    enigo
                        .move_mouse(tx, ty, Coordinate::Abs)
                        .map_err(|e| (ErrorCategory::OperationFailed, format!("Mouse move failed: {e}")))?;
                }
                enigo
                    .button(Button::Middle, Direction::Click)
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("Middle click failed: {e}")))?;
                let loc_str = match enigo.location() {
                    Ok((cur_x, cur_y)) => format!(" at (x: {cur_x}, y: {cur_y})"),
                    Err(_) => String::new(),
                };
                Ok((format!("Successfully performed middle click{loc_str}"), None))
            }
            "mouse_down" => {
                let btn = parse_button(&button_str)
                    .map_err(|e| (ErrorCategory::InvalidInput, e))?;
                if let (Some(tx), Some(ty)) = (x, y) {
                    enigo
                        .move_mouse(tx, ty, Coordinate::Abs)
                        .map_err(|e| (ErrorCategory::OperationFailed, format!("Mouse move failed: {e}")))?;
                }
                enigo
                    .button(btn, Direction::Press)
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("Mouse down failed: {e}")))?;
                Ok((format!("Mouse button '{button_str}' pressed down"), None))
            }
            "mouse_up" => {
                let btn = parse_button(&button_str)
                    .map_err(|e| (ErrorCategory::InvalidInput, e))?;
                if let (Some(tx), Some(ty)) = (x, y) {
                    enigo
                        .move_mouse(tx, ty, Coordinate::Abs)
                        .map_err(|e| (ErrorCategory::OperationFailed, format!("Mouse move failed: {e}")))?;
                }
                enigo
                    .button(btn, Direction::Release)
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("Mouse up failed: {e}")))?;
                Ok((format!("Mouse button '{button_str}' released"), None))
            }
            "drag" => {
                let dest_x = x.unwrap();
                let dest_y = y.unwrap();
                if let (Some(sx), Some(sy)) = (start_x, start_y) {
                    enigo
                        .move_mouse(sx, sy, Coordinate::Abs)
                        .map_err(|e| (ErrorCategory::OperationFailed, format!("Move to drag start failed: {e}")))?;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                enigo
                    .button(Button::Left, Direction::Press)
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("Press button for drag failed: {e}")))?;
                std::thread::sleep(std::time::Duration::from_millis(50));
                enigo
                    .move_mouse(dest_x, dest_y, Coordinate::Abs)
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("Drag move failed: {e}")))?;
                std::thread::sleep(std::time::Duration::from_millis(50));
                enigo
                    .button(Button::Left, Direction::Release)
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("Release button after drag failed: {e}")))?;
                Ok((
                    format!("Successfully dragged mouse to (x: {dest_x}, y: {dest_y})"),
                    None,
                ))
            }
            "type" => {
                let text_val = text.unwrap();
                enigo
                    .text(&text_val)
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("Typing text failed: {e}")))?;
                Ok((
                    format!("Successfully typed {} characters", text_val.chars().count()),
                    None,
                ))
            }
            "key" => {
                let full_key = key_str.unwrap();
                // Check if key is a combo like "Ctrl+c" or "Alt+F4"
                let parts: Vec<&str> = full_key.split('+').collect();
                let mut all_modifiers = modifier_keys;
                let target_key_str = if parts.len() > 1 {
                    for part in &parts[..parts.len() - 1] {
                        let m = parse_modifier(part).map_err(|e| (ErrorCategory::InvalidInput, e))?;
                        if !all_modifiers.contains(&m) {
                            all_modifiers.push(m);
                        }
                    }
                    parts[parts.len() - 1]
                } else {
                    full_key.as_str()
                };

                let target_key = parse_key(target_key_str)
                    .map_err(|e| (ErrorCategory::InvalidInput, e))?;

                for modifier in &all_modifiers {
                    enigo
                        .key(*modifier, Direction::Press)
                        .map_err(|e| (ErrorCategory::OperationFailed, format!("Pressing modifier failed: {e}")))?;
                }
                enigo
                    .key(target_key, Direction::Click)
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("Pressing key failed: {e}")))?;
                for modifier in all_modifiers.iter().rev() {
                    let _ = enigo.key(*modifier, Direction::Release);
                }

                Ok((format!("Successfully pressed key '{full_key}'"), None))
            }
            "scroll" => {
                let amount = scroll_amount.unwrap();
                let axis = match axis_str.to_ascii_lowercase().as_str() {
                    "horizontal" => Axis::Horizontal,
                    _ => Axis::Vertical,
                };
                enigo
                    .scroll(amount, axis)
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("Scroll failed: {e}")))?;
                Ok((
                    format!("Successfully scrolled {axis_str} by {amount} clicks"),
                    None,
                ))
            }
            "cursor_position" => {
                let (cur_x, cur_y) = enigo
                    .location()
                    .map_err(|e| (ErrorCategory::OperationFailed, format!("Failed to retrieve cursor position: {e}")))?;
                Ok((
                    format!("Current cursor position: x={cur_x}, y={cur_y}"),
                    Some(serde_json::json!({ "x": cur_x, "y": cur_y })),
                ))
            }
            _ => unreachable!(),
        }
    })
    .await;

    match join_res {
        Ok(exec_result) => match exec_result {
            Ok((msg, data)) => {
                let msg = match (mapping, raw_x, raw_y, x, y) {
                    (Some(_), Some(ix), Some(iy), Some(ax), Some(ay)) if (ix, iy) != (ax, ay) => {
                        format!(
                            "{msg}\nConverted image-local ({ix}, {iy}) → absolute ({ax}, {ay})."
                        )
                    }
                    _ => msg,
                };
                let mut res = MCPResult::success(&msg);
                if let Some(data) = data {
                    res.structured_content = Some(data);
                } else if let (Some(ax), Some(ay)) = (x, y) {
                    res.structured_content = Some(serde_json::json!({
                        "x": ax,
                        "y": ay,
                        "image_x": raw_x,
                        "image_y": raw_y,
                        "coordinate_space": if mapping.is_some() { "image_local_input" } else { "absolute" }
                    }));
                }
                Ok(res)
            }
            Err((category, err_msg)) => {
                Ok(guided_error(category, err_msg, ToolGroup::Desktop).to_mcp_result())
            }
        },
        Err(join_err) => Ok(guided_error(
            ErrorCategory::InternalError,
            format!("Desktop control execution task failed: {join_err}"),
            ToolGroup::Desktop,
        )
        .to_mcp_result()),
    }
}
