use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

/// Returns all tools provided by the Media server.
pub fn all_tools() -> Vec<MCPTool> {
    vec![see_tool(), listen_tool(), capture_screen_tool()]
}

fn see_tool() -> MCPTool {
    MCPTool {
        name: "seeContent".to_string(),
        title: Some("See Content".to_string()),
        description: r#"Fetch an image and include it in the conversation so you can visually analyse it.

**Supported formats:** JPEG, PNG, GIF, WebP, BMP, SVG

**Notes:**
- Maximum file size: 20 MB.
- Local paths must be inside the session workspace."#
            .to_string(),
        input_schema: object_prop(
            vec![(
                "url".to_string(),
                string_prop_required(
                    "URL or workspace-relative path of the image to fetch (e.g. https://example.com/photo.jpg or screenshots/capture.png).",
                ),
            )],
            vec!["url".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}

fn listen_tool() -> MCPTool {
    MCPTool {
        name: "listenContent".to_string(),
        title: Some("Listen Content".to_string()),
        description: r#"Fetch an audio file and include it in the conversation so you can analyse the audio.

**Supported formats:** MP3, WAV, OGG, AAC, FLAC, WEBM

**Notes:**
- Maximum file size: 20 MB.
- Local paths must be inside the session workspace."#
            .to_string(),
        input_schema: object_prop(
            vec![(
                "url".to_string(),
                string_prop_required(
                    "URL or workspace-relative path of the audio file to fetch (e.g. https://example.com/clip.mp3 or recordings/audio.wav).",
                ),
            )],
            vec!["url".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}

fn capture_screen_tool() -> MCPTool {
    MCPTool {
        name: "captureScreen".to_string(),
        title: Some("Capture Screen".to_string()),
        description: r#"Capture the live desktop screen or a specific screen region and include the screenshot in the conversation as an image for visual analysis.

**Parameters:**
- `display_index` (optional integer, default: 0): Zero-based index of the display monitor to capture.
- `x` (optional integer): X coordinate of the top-left corner for area capture.
- `y` (optional integer): Y coordinate of the top-left corner for area capture.
- `width` (optional integer): Width in pixels of the area to capture (minimum: 1).
- `height` (optional integer): Height in pixels of the area to capture (minimum: 1).

**Notes & Caveats:**
- To capture a specific sub-region, ALL four parameters (`x`, `y`, `width`, `height`) must be provided. Partial region arguments will be rejected as an error.
- Region `x`/`y` are image-local to the selected monitor capture (0,0 = top-left of that monitor), not virtual-desktop absolute coordinates.
- If region parameters are omitted, the full display at `display_index` will be captured.
- Maximum payload size is 20 MB.
- Operating system permissions: Requires screen-recording permission if restricted by the OS (e.g., macOS or Wayland).
- After capture, click with `desktop__computerControl` using the same `display_index`, raw image-pixel `x`/`y` (do not divide by DPI/`scale_factor`), and copy `width_scale`/`height_scale` from this tool's structured content. Conversion to absolute input coordinates is performed by the desktop tool."#
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "display_index".to_string(),
                    integer_prop_with_default(
                        Some(0),
                        None,
                        0,
                        Some("Zero-based display monitor index (default: 0)."),
                    ),
                ),
                (
                    "x".to_string(),
                    integer_prop(
                        None,
                        None,
                        Some("X coordinate of the top-left corner for area capture (requires y, width, and height)."),
                    ),
                ),
                (
                    "y".to_string(),
                    integer_prop(
                        None,
                        None,
                        Some("Y coordinate of the top-left corner for area capture (requires x, width, and height)."),
                    ),
                ),
                (
                    "width".to_string(),
                    integer_prop(
                        Some(1),
                        None,
                        Some("Width in pixels of the area to capture (minimum: 1, requires x, y, and height)."),
                    ),
                ),
                (
                    "height".to_string(),
                    integer_prop(
                        Some(1),
                        None,
                        Some("Height in pixels of the area to capture (minimum: 1, requires x, y, and width)."),
                    ),
                ),
            ],
            vec![],
            None,
        ),
        output_schema: None,
        annotations: None,
        libragent_wait: None,
    }
}
