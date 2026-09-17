use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use std::path::Path;
use url::Url;

use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ToolGroup,
};
use crate::mcp::types::{MCPContent, MCPResult};
use image::ImageFormat;
use xcap::Monitor;

/// Maximum allowed download size (20 MB).
const MAX_BYTES: usize = 20 * 1024 * 1024;

// ── MIME helpers ──────────────────────────────────────────────────────────────

/// Detect image MIME type from a file extension.
pub fn image_mime_from_ext(ext: &str) -> Option<&'static str> {
    match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        "ico" => Some("image/x-icon"),
        _ => None,
    }
}

/// Detect audio MIME type from a file extension.
pub fn audio_mime_from_ext(ext: &str) -> Option<&'static str> {
    match ext.to_lowercase().as_str() {
        "mp3" => Some("audio/mpeg"),
        "wav" => Some("audio/wav"),
        "ogg" => Some("audio/ogg"),
        "aac" => Some("audio/aac"),
        "flac" => Some("audio/flac"),
        "webm" => Some("audio/webm"),
        "m4a" => Some("audio/mp4"),
        _ => None,
    }
}

/// Extract the file extension from a URL path or local path string.
fn ext_from_url_path(url: &str) -> Option<String> {
    // Strip query string and fragment before extracting extension.
    let path_part = url.split('?').next().unwrap_or(url);
    let path_part = path_part.split('#').next().unwrap_or(path_part);
    Path::new(path_part)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
}

/// Determine the MIME type for an image given a URL/path and an optional
/// Content-Type header value from an HTTP response.
pub fn resolve_image_mime(url: &str, content_type_header: Option<&str>) -> Option<String> {
    // Prefer the HTTP Content-Type header when it is present and specific.
    if let Some(ct) = content_type_header {
        let ct = ct.split(';').next().unwrap_or(ct).trim().to_lowercase();
        if ct.starts_with("image/") {
            return Some(ct);
        }
    }
    // Fall back to file extension.
    ext_from_url_path(url).and_then(|ext| image_mime_from_ext(&ext).map(|s| s.to_string()))
}

/// Determine the MIME type for audio given a URL/path and an optional
/// Content-Type header value from an HTTP response.
pub fn resolve_audio_mime(url: &str, content_type_header: Option<&str>) -> Option<String> {
    if let Some(ct) = content_type_header {
        let ct = ct.split(';').next().unwrap_or(ct).trim().to_lowercase();
        if ct.starts_with("audio/") {
            return Some(ct);
        }
    }
    ext_from_url_path(url).and_then(|ext| audio_mime_from_ext(&ext).map(|s| s.to_string()))
}

// ── Source resolution ─────────────────────────────────────────────────────────

/// Describes where the content bytes come from.
pub enum ContentSource {
    /// An HTTP/HTTPS URL to be fetched with reqwest.
    Http(String),
    /// An absolute or resolved local file path.
    LocalFile(std::path::PathBuf),
}

/// Parse the `url` argument into a `ContentSource`.
///
/// Accepts:
/// - `https://…` / `http://…`  → `Http`
/// - `file:///path`             → `LocalFile`
/// - Any other string           → treated as a local path → `LocalFile`
pub fn parse_source(url: &str) -> Result<ContentSource, String> {
    if url.starts_with("https://") || url.starts_with("http://") {
        Ok(ContentSource::Http(url.to_string()))
    } else if url.starts_with("file://") {
        let parsed = Url::parse(url).map_err(|e| format!("Invalid file URL format: {e}"))?;
        let path = parsed
            .to_file_path()
            .map_err(|_| "URL cannot be converted to a local file path".to_string())?;
        Ok(ContentSource::LocalFile(path))
    } else {
        Ok(ContentSource::LocalFile(std::path::PathBuf::from(url)))
    }
}

// ── Fetch helpers ─────────────────────────────────────────────────────────────

/// Fetch bytes from an HTTP/HTTPS URL.
/// Returns `(bytes, content_type_header_value)`.
pub async fn fetch_http_bytes(url: &str) -> Result<(Vec<u8>, Option<String>), String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; LibrAgent/1.0)")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {} for URL: {}", response.status(), url));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Check Content-Length before downloading to avoid excessive memory use.
    if let Some(len) = response.content_length() {
        if len as usize > MAX_BYTES {
            return Err(format!(
                "Remote file is too large ({} bytes). Maximum allowed size is {} MB.",
                len,
                MAX_BYTES / 1024 / 1024
            ));
        }
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "Downloaded content is too large ({} bytes). Maximum allowed size is {} MB.",
            bytes.len(),
            MAX_BYTES / 1024 / 1024
        ));
    }

    Ok((bytes.to_vec(), content_type))
}

/// Read bytes from a local file, enforcing the size cap.
pub async fn read_local_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|e| format!("Cannot access file '{}': {e}", path.display()))?;

    if metadata.len() as usize > MAX_BYTES {
        return Err(format!(
            "File is too large ({} bytes). Maximum allowed size is {} MB.",
            metadata.len(),
            MAX_BYTES / 1024 / 1024
        ));
    }

    tokio::fs::read(path)
        .await
        .map_err(|e| format!("Failed to read file '{}': {e}", path.display()))
}

// ── Handler: seeContent ───────────────────────────────────────────────────────

/// Resolve a local path against the session workspace when it is relative.
fn resolve_local_path(path: &Path, workspace_dir: &Path) -> std::path::PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_dir.join(path)
    }
}

/// Validate that a resolved local path does not escape the workspace.
fn ensure_within_workspace(path: &Path, workspace_dir: &Path) -> Result<(), String> {
    let canonical_path = path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve path '{}': {e}", path.display()))?;
    let canonical_workspace = workspace_dir
        .canonicalize()
        .map_err(|e| format!("Cannot resolve workspace directory: {e}"))?;

    if canonical_path.starts_with(&canonical_workspace) {
        Ok(())
    } else {
        Err(format!(
            "Access denied: '{}' is outside the session workspace.",
            path.display()
        ))
    }
}

/// Handle the `seeContent` tool.
pub async fn handle_see_content(
    args: Value,
    workspace_dir: std::path::PathBuf,
) -> Result<MCPResult, String> {
    let url_str = match args.get("url").and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => return Ok(missing_param_error("url", ToolGroup::Media)),
    };

    let (bytes, content_type_header) = {
        let source = match parse_source(&url_str) {
            Ok(s) => s,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Invalid source '{url_str}': {e}"),
                    ToolGroup::Media,
                )
                .to_mcp_result())
            }
        };
        match source {
            ContentSource::Http(url) => match fetch_http_bytes(&url).await {
                Ok(pair) => pair,
                Err(e) => {
                    return Ok(guided_error(
                        ErrorCategory::OperationFailed,
                        format!("Failed to fetch from '{url_str}': {e}"),
                        ToolGroup::Media,
                    )
                    .to_mcp_result())
                }
            },
            ContentSource::LocalFile(raw_path) => {
                let resolved = resolve_local_path(&raw_path, &workspace_dir);
                if let Err(e) = ensure_within_workspace(&resolved, &workspace_dir) {
                    return Ok(
                        guided_error(ErrorCategory::PermissionDenied, e, ToolGroup::Media)
                            .to_mcp_result(),
                    );
                }
                match read_local_bytes(&resolved).await {
                    Ok(data) => (data, None),
                    Err(e) => {
                        return Ok(guided_error(
                            ErrorCategory::ResourceNotFound,
                            e,
                            ToolGroup::Media,
                        )
                        .to_mcp_result())
                    }
                }
            }
        }
    };

    let mime_type = match resolve_image_mime(&url_str, content_type_header.as_deref()) {
        Some(m) => m,
        None => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Could not determine image MIME type for '{url_str}'. \
                     Ensure the URL/path points to a supported image format \
                     (JPEG, PNG, GIF, WebP, BMP, SVG)."
                ),
                ToolGroup::Media,
            )
            .to_mcp_result());
        }
    };

    if !mime_type.starts_with("image/") {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!("URL does not point to an image (detected MIME type: {mime_type})."),
            ToolGroup::Media,
        )
        .to_mcp_result());
    }

    let data = general_purpose::STANDARD.encode(&bytes);
    let size_kb = bytes.len() / 1024;

    Ok(MCPResult {
        content: Some(vec![
            MCPContent::Text {
                text: format!("✓ Image loaded ({size_kb} KB, {mime_type})\n\nSource: {url_str}"),
            },
            MCPContent::Image {
                data: Some(data),
                uri: None,
                mime_type,
            },
        ]),
        structured_content: None,
        is_error: Some(false),
    })
}

// ── Handler: listenContent ────────────────────────────────────────────────────

/// Handle the `listenContent` tool.
pub async fn handle_listen_content(
    args: Value,
    workspace_dir: std::path::PathBuf,
) -> Result<MCPResult, String> {
    let url_str = match args.get("url").and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => return Ok(missing_param_error("url", ToolGroup::Media)),
    };

    let (bytes, content_type_header) = {
        let source = match parse_source(&url_str) {
            Ok(s) => s,
            Err(e) => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Invalid source '{url_str}': {e}"),
                    ToolGroup::Media,
                )
                .to_mcp_result())
            }
        };
        match source {
            ContentSource::Http(url) => match fetch_http_bytes(&url).await {
                Ok(pair) => pair,
                Err(e) => {
                    return Ok(guided_error(
                        ErrorCategory::OperationFailed,
                        format!("Failed to fetch from '{url_str}': {e}"),
                        ToolGroup::Media,
                    )
                    .to_mcp_result())
                }
            },
            ContentSource::LocalFile(raw_path) => {
                let resolved = resolve_local_path(&raw_path, &workspace_dir);
                if let Err(e) = ensure_within_workspace(&resolved, &workspace_dir) {
                    return Ok(
                        guided_error(ErrorCategory::PermissionDenied, e, ToolGroup::Media)
                            .to_mcp_result(),
                    );
                }
                match read_local_bytes(&resolved).await {
                    Ok(data) => (data, None),
                    Err(e) => {
                        return Ok(guided_error(
                            ErrorCategory::ResourceNotFound,
                            e,
                            ToolGroup::Media,
                        )
                        .to_mcp_result())
                    }
                }
            }
        }
    };

    let mime_type = match resolve_audio_mime(&url_str, content_type_header.as_deref()) {
        Some(m) => m,
        None => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!(
                    "Could not determine audio MIME type for '{url_str}'. \
                     Ensure the URL/path points to a supported audio format \
                     (MP3, WAV, OGG, AAC, FLAC, WEBM)."
                ),
                ToolGroup::Media,
            )
            .to_mcp_result());
        }
    };

    if !mime_type.starts_with("audio/") {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!("URL does not point to an audio file (detected MIME type: {mime_type})."),
            ToolGroup::Media,
        )
        .to_mcp_result());
    }

    let data = general_purpose::STANDARD.encode(&bytes);
    let size_kb = bytes.len() / 1024;

    Ok(MCPResult {
        content: Some(vec![
            MCPContent::Text {
                text: format!("✓ Audio loaded ({size_kb} KB, {mime_type})\n\nSource: {url_str}"),
            },
            MCPContent::Audio {
                data: Some(data),
                uri: None,
                mime_type,
            },
        ]),
        structured_content: None,
        is_error: Some(false),
    })
}

// ── Handler: captureScreen ───────────────────────────────────────────────────

fn screen_capture_guidance() -> Vec<String> {
    vec![
        "Verify that 'display_index' corresponds to an active monitor (use 0 for primary display).".to_string(),
        "If capturing a region, ensure 'x', 'y', 'width', and 'height' are all specified and within the captured monitor image bounds (image-local to that monitor).".to_string(),
        "Check operating system screen recording permissions (e.g. macOS System Settings > Privacy & Security, or Wayland compositor permissions).".to_string(),
        "Omit region coordinates to capture the entire display.".to_string(),
        "Click with desktop__computerControl using the same display_index and image-pixel x/y from this screenshot — the desktop tool converts to absolute coordinates.".to_string(),
    ]
}

/// Successful capture payload returned from the blocking worker.
struct ScreenCapturePayload {
    png_data: Vec<u8>,
    width: u32,
    height: u32,
    target_desc: String,
    /// Absolute virtual-desktop X of image pixel (0, 0).
    origin_x: i32,
    /// Absolute virtual-desktop Y of image pixel (0, 0).
    origin_y: i32,
    /// Multiply image-local X by this to convert to absolute delta (`monitor_w / image_w`).
    width_scale: f64,
    /// Multiply image-local Y by this to convert to absolute delta (`monitor_h / image_h`).
    height_scale: f64,
    scale_factor: f32,
    monitor_width: u32,
    monitor_height: u32,
}

type ScreenCaptureError = (ErrorCategory, String);
type ScreenCaptureTaskResult = Result<ScreenCapturePayload, ScreenCaptureError>;

/// Handle the `captureScreen` tool.
pub async fn handle_capture_screen(args: Value) -> Result<MCPResult, String> {
    // 1. Validate display_index
    let display_index = if let Some(val) = args.get("display_index") {
        if let Some(i) = val.as_i64() {
            if i < 0 {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    format!("Parameter 'display_index' must be a non-negative integer, got {i}."),
                    ToolGroup::Media,
                )
                .with_guidance(screen_capture_guidance())
                .to_mcp_result());
            }
            i as usize
        } else {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Parameter 'display_index' must be an integer.".to_string(),
                ToolGroup::Media,
            )
            .with_guidance(screen_capture_guidance())
            .to_mcp_result());
        }
    } else {
        0
    };

    // 2. Validate region coordinates
    let has_x = args.get("x").is_some();
    let has_y = args.get("y").is_some();
    let has_w = args.get("width").is_some();
    let has_h = args.get("height").is_some();

    let region = if has_x || has_y || has_w || has_h {
        if !(has_x && has_y && has_w && has_h) {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                "Incomplete region parameters. When capturing a specific area, all four parameters ('x', 'y', 'width', 'height') must be provided.",
                ToolGroup::Media,
            )
            .with_guidance(screen_capture_guidance())
            .to_mcp_result());
        }

        let x = match args.get("x").and_then(|v| v.as_i64()) {
            Some(v) if v >= i32::MIN as i64 && v <= i32::MAX as i64 => v as i32,
            _ => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Parameter 'x' must be a valid 32-bit integer coordinate.",
                    ToolGroup::Media,
                )
                .with_guidance(screen_capture_guidance())
                .to_mcp_result());
            }
        };

        let y = match args.get("y").and_then(|v| v.as_i64()) {
            Some(v) if v >= i32::MIN as i64 && v <= i32::MAX as i64 => v as i32,
            _ => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Parameter 'y' must be a valid 32-bit integer coordinate.",
                    ToolGroup::Media,
                )
                .with_guidance(screen_capture_guidance())
                .to_mcp_result());
            }
        };

        let width = match args.get("width").and_then(|v| v.as_i64()) {
            Some(v) if v > 0 && v <= u32::MAX as i64 => v as u32,
            _ => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Parameter 'width' must be a positive integer (greater than 0).",
                    ToolGroup::Media,
                )
                .with_guidance(screen_capture_guidance())
                .to_mcp_result());
            }
        };

        let height = match args.get("height").and_then(|v| v.as_i64()) {
            Some(v) if v > 0 && v <= u32::MAX as i64 => v as u32,
            _ => {
                return Ok(guided_error(
                    ErrorCategory::InvalidInput,
                    "Parameter 'height' must be a positive integer (greater than 0).",
                    ToolGroup::Media,
                )
                .with_guidance(screen_capture_guidance())
                .to_mcp_result());
            }
        };

        Some((x, y, width, height))
    } else {
        None
    };

    // 3. Perform blocking screen capture
    let capture_task = tokio::task::spawn_blocking(move || -> ScreenCaptureTaskResult {
        let monitors = Monitor::all().map_err(|e| {
            (
                ErrorCategory::OperationFailed,
                format!("Failed to enumerate display monitors: {e}"),
            )
        })?;

        if monitors.is_empty() {
            return Err((
                ErrorCategory::ResourceNotFound,
                "No active display monitors found on the system.".to_string(),
            ));
        }

        let monitor = monitors.get(display_index).ok_or_else(|| {
            (
                ErrorCategory::InvalidInput,
                format!(
                    "Display index {} is out of range. Available displays: 0 to {}.",
                    display_index,
                    monitors.len().saturating_sub(1)
                ),
            )
        })?;

        let monitor_x = monitor.x().map_err(|e| {
            (
                ErrorCategory::OperationFailed,
                format!("Failed to read monitor X origin for display {display_index}: {e}"),
            )
        })?;
        let monitor_y = monitor.y().map_err(|e| {
            (
                ErrorCategory::OperationFailed,
                format!("Failed to read monitor Y origin for display {display_index}: {e}"),
            )
        })?;
        let monitor_width = monitor.width().map_err(|e| {
            (
                ErrorCategory::OperationFailed,
                format!("Failed to read monitor width for display {display_index}: {e}"),
            )
        })?;
        let monitor_height = monitor.height().map_err(|e| {
            (
                ErrorCategory::OperationFailed,
                format!("Failed to read monitor height for display {display_index}: {e}"),
            )
        })?;
        let scale_factor = monitor.scale_factor().unwrap_or(1.0);

        let mut full_image = monitor.capture_image().map_err(|e| {
            (
                ErrorCategory::OperationFailed,
                format!("Failed to capture display {display_index}: {e}"),
            )
        })?;

        let full_w = full_image.width().max(1);
        let full_h = full_image.height().max(1);
        let width_scale = monitor_width as f64 / full_w as f64;
        let height_scale = monitor_height as f64 / full_h as f64;

        let (image, desc, origin_x, origin_y) = match region {
            Some((x, y, w, h)) => {
                if x < 0
                    || y < 0
                    || (x as u32).saturating_add(w) > full_w
                    || (y as u32).saturating_add(h) > full_h
                {
                    return Err((
                        ErrorCategory::InvalidInput,
                        format!(
                            "Screen area ({x}, {y}, {w}x{h}) exceeds monitor image bounds ({full_w}x{full_h}) on display {display_index}. Region x/y are image-local (0,0 = top-left of this monitor capture), not virtual-desktop absolute."
                        ),
                    ));
                }

                let cropped =
                    image::imageops::crop(&mut full_image, x as u32, y as u32, w, h).to_image();
                let origin_x = monitor_x + ((x as f64) * width_scale).round() as i32;
                let origin_y = monitor_y + ((y as f64) * height_scale).round() as i32;
                (
                    cropped,
                    format!(
                        "Screen area image-local ({x}, {y}, {w}x{h}) on display {display_index}; absolute origin ({origin_x}, {origin_y})"
                    ),
                    origin_x,
                    origin_y,
                )
            }
            None => (
                full_image,
                format!(
                    "Display {display_index} image ({full_w}x{full_h}); absolute origin ({monitor_x}, {monitor_y}); monitor logical size {monitor_width}x{monitor_height}"
                ),
                monitor_x,
                monitor_y,
            ),
        };

        let width = image.width();
        let height = image.height();

        let mut png_bytes = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut png_bytes, ImageFormat::Png)
            .map_err(|e| {
                (
                    ErrorCategory::OperationFailed,
                    format!("Failed to encode screenshot as PNG: {e}"),
                )
            })?;

        let data = png_bytes.into_inner();
        if data.len() > MAX_BYTES {
            return Err((
                ErrorCategory::OperationFailed,
                format!(
                    "Captured screenshot size ({:.1} MB) exceeds maximum allowed payload of {} MB.",
                    data.len() as f64 / (1024.0 * 1024.0),
                    MAX_BYTES / (1024 * 1024)
                ),
            ));
        }

        Ok(ScreenCapturePayload {
            png_data: data,
            width,
            height,
            target_desc: desc,
            origin_x,
            origin_y,
            width_scale,
            height_scale,
            scale_factor,
            monitor_width,
            monitor_height,
        })
    });

    let capture_result = match capture_task.await {
        Ok(res) => res,
        Err(join_err) => {
            return Ok(guided_error(
                ErrorCategory::OperationFailed,
                format!("Screen capture worker task panicked or failed: {join_err}"),
                ToolGroup::Media,
            )
            .with_guidance(screen_capture_guidance())
            .to_mcp_result());
        }
    };

    match capture_result {
        Ok(payload) => {
            let byte_len = payload.png_data.len();
            let size_kb = byte_len / 1024;
            let base64_data = general_purpose::STANDARD.encode(&payload.png_data);
            let ScreenCapturePayload {
                png_data: _,
                width,
                height,
                target_desc,
                origin_x,
                origin_y,
                width_scale,
                height_scale,
                scale_factor,
                monitor_width,
                monitor_height,
            } = payload;

            Ok(MCPResult {
                content: Some(vec![
                    MCPContent::Text {
                        text: format!(
                            "✓ Screenshot captured ({width}x{height}, {size_kb} KB, image/png)\n\n\
                             Target: {target_desc}\n\n\
                             To click a point you see in this image, call desktop__computerControl with \
                             display_index={display_index} and the image-pixel x/y (0,0 = top-left of this image). \
                             Conversion to absolute screen coordinates is done by the desktop tool.\n\
                             Cropped-capture origin (if needed): origin_x={origin_x}, origin_y={origin_y}."
                        ),
                    },
                    MCPContent::Image {
                        data: Some(base64_data),
                        uri: None,
                        mime_type: "image/png".to_string(),
                    },
                ]),
                structured_content: Some(serde_json::json!({
                    "display_index": display_index,
                    "width": width,
                    "height": height,
                    "bytes": byte_len,
                    "mime_type": "image/png",
                    "origin_x": origin_x,
                    "origin_y": origin_y,
                    "width_scale": width_scale,
                    "height_scale": height_scale,
                    "scale_factor": scale_factor,
                    "monitor_width": monitor_width,
                    "monitor_height": monitor_height,
                    "coordinate_space": "image_local"
                })),
                is_error: Some(false),
            })
        }
        Err((category, err_msg)) => Ok(guided_error(category, err_msg, ToolGroup::Media)
            .with_guidance(screen_capture_guidance())
            .to_mcp_result()),
    }
}
