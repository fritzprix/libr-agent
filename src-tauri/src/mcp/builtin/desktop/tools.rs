use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

/// Returns all tools provided by the Desktop server.
pub fn all_tools() -> Vec<MCPTool> {
    vec![computer_control_tool()]
}

fn computer_control_tool() -> MCPTool {
    MCPTool {
        name: "computerControl".to_string(),
        title: Some("Computer Control".to_string()),
        description: r#"Interact with the desktop operating system via mouse and keyboard simulation.

**Supported actions:**
- `click`: Click the mouse button at current cursor location, or at (x, y) if coordinates are provided.
- `double_click`: Double click the mouse button at current cursor location or at (x, y).
- `right_click`: Right click at current location or at (x, y).
- `middle_click`: Middle click at current location or at (x, y).
- `move`: Move the mouse cursor to coordinates (x, y).
- `mouse_down`: Press down a mouse button without releasing.
- `mouse_up`: Release a previously pressed mouse button.
- `drag`: Drag the mouse to (x, y) with left button held down. Optional (start_x, start_y) can be specified.
- `type`: Type a string of text.
- `key`: Press an individual key or key combination (e.g., "Return", "Escape", "Tab", "Ctrl+c", "Alt+F4").
- `scroll`: Scroll vertically or horizontally by a given amount.
- `cursor_position`: Query and return current mouse cursor coordinates (absolute).

**Coordinates (important):**
- Preferred workflow: call `media__captureScreen` with a `display_index`, then pass the **same** `display_index` plus **image-pixel** `x`/`y` from that screenshot (0,0 = top-left of the image). This tool converts image coordinates to absolute OS input coordinates.
- For cropped screenshots, also pass `origin_x`/`origin_y` (and optional `width_scale`/`height_scale`) from the capture response.
- If `display_index` / `origin_*` are omitted, `x`/`y` are treated as absolute virtual-desktop coordinates (may be negative on multi-monitor layouts).

**Notes & Best Practices:**
- Linux requirement: Requires an active X11 or Xwayland display session ($DISPLAY).
- Caution with `mouse_down`: Leaves the button pressed until `mouse_up` or a `click` action is invoked. Always pair `mouse_down` with `mouse_up`.
- Key combinations can be specified either in `key` (e.g., "Ctrl+v") or with `modifiers` (e.g., key: "v", modifiers: ["ctrl"])."#
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "action".to_string(),
                    enum_prop_required(
                        vec![
                            "click",
                            "double_click",
                            "right_click",
                            "middle_click",
                            "move",
                            "mouse_down",
                            "mouse_up",
                            "drag",
                            "type",
                            "key",
                            "scroll",
                            "cursor_position",
                        ],
                        "Action to perform on the desktop system.",
                    ),
                ),
                (
                    "display_index".to_string(),
                    integer_prop(
                        Some(0),
                        None,
                        Some(
                            "Monitor index from media__captureScreen. When set, x/y are image-pixel coordinates for that display and are converted to absolute input coordinates.",
                        ),
                    ),
                ),
                (
                    "origin_x".to_string(),
                    integer_prop(
                        None,
                        None,
                        Some(
                            "Absolute X of image pixel (0,0) from media__captureScreen. Use with origin_y for cropped captures (optional if display_index is set for a full-display capture).",
                        ),
                    ),
                ),
                (
                    "origin_y".to_string(),
                    integer_prop(
                        None,
                        None,
                        Some(
                            "Absolute Y of image pixel (0,0) from media__captureScreen. Use with origin_x for cropped captures.",
                        ),
                    ),
                ),
                (
                    "width_scale".to_string(),
                    number_prop(
                        None,
                        None,
                        Some(
                            "Optional image→absolute X scale from media__captureScreen structured content (defaults to 1/scale_factor for display_index mode).",
                        ),
                    ),
                ),
                (
                    "height_scale".to_string(),
                    number_prop(
                        None,
                        None,
                        Some(
                            "Optional image→absolute Y scale from media__captureScreen structured content.",
                        ),
                    ),
                ),
                (
                    "x".to_string(),
                    integer_prop(
                        None,
                        None,
                        Some(
                            "Target X for move/click/drag. Image-pixel when display_index or origin_* is set; otherwise absolute virtual-desktop X.",
                        ),
                    ),
                ),
                (
                    "y".to_string(),
                    integer_prop(
                        None,
                        None,
                        Some(
                            "Target Y for move/click/drag. Image-pixel when display_index or origin_* is set; otherwise absolute virtual-desktop Y.",
                        ),
                    ),
                ),
                (
                    "start_x".to_string(),
                    integer_prop(
                        None,
                        None,
                        Some("Starting X for drag (same coordinate space as x). If omitted, drag starts at current cursor position."),
                    ),
                ),
                (
                    "start_y".to_string(),
                    integer_prop(
                        None,
                        None,
                        Some("Starting Y for drag (same coordinate space as y). If omitted, drag starts at current cursor position."),
                    ),
                ),
                (
                    "button".to_string(),
                    enum_prop(
                        vec!["left", "right", "middle"],
                        "left",
                        Some("Mouse button for click, double_click, mouse_down, mouse_up (default: 'left')."),
                    ),
                ),
                (
                    "text".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Text string to type (required for action: 'type')."),
                    ),
                ),
                (
                    "key".to_string(),
                    string_prop(
                        None,
                        None,
                        Some("Key name to press (for action: 'key'). Examples: 'Return', 'Enter', 'Tab', 'Space', 'Backspace', 'Escape', 'F5', 'Up', 'Down', 'Left', 'Right', 'Ctrl+c', 'Alt+F4', or single character 'a'."),
                    ),
                ),
                (
                    "modifiers".to_string(),
                    array_schema(
                        string_prop(None, None, Some("Modifier key name ('ctrl', 'alt', 'shift', 'meta')")),
                        Some("Optional modifier keys to hold while pressing the key or clicking."),
                    ),
                ),
                (
                    "scroll_amount".to_string(),
                    integer_prop(
                        None,
                        None,
                        Some("Number of scroll clicks/steps (for action: 'scroll'). Positive scrolls down or right; negative scrolls up or left."),
                    ),
                ),
                (
                    "axis".to_string(),
                    enum_prop(
                        vec!["vertical", "horizontal"],
                        "vertical",
                        Some("Scroll axis for action 'scroll' (default: 'vertical')."),
                    ),
                ),
            ],
            vec!["action".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}
