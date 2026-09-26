use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use url::Url;

use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ToolGroup,
};
use crate::mcp::types::{MCPContent, MCPResult};
use image::ImageFormat;
use xcap::Monitor;

/// Maximum allowed download size (20 MB).
const MAX_BYTES: usize = 20 * 1024 * 1024;

/// Hard cap on successful `seeContent` image loads per media-server session.
///
/// Soft tool-description advice alone does not stop all-frame multimodal dumps;
/// the handler enforces this bound and returns guided recovery.
pub const MAX_SEE_CONTENT_SUCCESSES_PER_SESSION: usize = 8;

/// RAII reservation for one `seeContent` success slot.
///
/// Drop releases the slot unless [`SeeContentSlot::commit`] was called (load
/// succeeded and multimodal content will be returned).
struct SeeContentSlot<'a> {
    counter: &'a AtomicUsize,
    committed: bool,
}

impl<'a> SeeContentSlot<'a> {
    fn try_reserve(counter: &'a AtomicUsize) -> Result<Self, usize> {
        let prev = counter.fetch_add(1, Ordering::SeqCst);
        if prev >= MAX_SEE_CONTENT_SUCCESSES_PER_SESSION {
            counter.fetch_sub(1, Ordering::SeqCst);
            return Err(MAX_SEE_CONTENT_SUCCESSES_PER_SESSION);
        }
        Ok(Self {
            counter,
            committed: false,
        })
    }

    fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for SeeContentSlot<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.counter.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

fn see_content_cap_recovery() -> Vec<String> {
    vec![
        format!(
            "This session already used the maximum of {MAX_SEE_CONTENT_SUCCESSES_PER_SESSION} successful seeContent loads."
        ),
        "Do not dump every video frame or large image set into context — sample a few key frames only."
            .to_string(),
        "For speech-in-video: extract audio (wav/mp3) and call media__listenContent on that file instead of more seeContent."
            .to_string(),
        "If scripting tools (ffmpeg/python) are available, filter or OCR offline, then use seeContent only for final verification samples."
            .to_string(),
    ]
}

fn see_content_cap_exceeded_result() -> MCPResult {
    guided_error(
        ErrorCategory::InvalidState,
        format!(
            "seeContent session limit reached ({MAX_SEE_CONTENT_SUCCESSES_PER_SESSION} successful image loads). Further seeContent calls are blocked for this session."
        ),
        ToolGroup::Media,
    )
    .with_guidance(see_content_cap_recovery())
    .to_mcp_result()
}

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

/// True when the path/URL extension looks like a video container (not audio).
///
/// Used to tailor `listenContent` recovery — `webm` is omitted because it is a
/// supported audio extension for this tool.
pub fn looks_like_video_path(url: &str) -> bool {
    matches!(
        ext_from_url_path(url).as_deref(),
        Some("mp4" | "mkv" | "mov" | "avi" | "m4v" | "mpeg" | "mpg" | "wmv")
    )
}

fn listen_content_recovery(url: &str) -> Vec<String> {
    if looks_like_video_path(url) {
        vec![
            "This path looks like a video container; listenContent accepts audio only."
                .to_string(),
            "If ffmpeg is available: extract audio (e.g. `ffmpeg -y -i <video> -vn -acodec pcm_s16le /app/audio.wav`), then call media__listenContent on that wav/mp3."
                .to_string(),
            "If ffmpeg is missing, do not apt-install for long stretches — sample a few keyframes and use media__seeContent on those images instead."
                .to_string(),
            "Prefer media__listenContent / media__seeContent over installing OCR/ASR stacks when the goal is transcription or on-screen text."
                .to_string(),
        ]
    } else {
        vec![
            "Provide a URL/path to a supported audio format (MP3, WAV, OGG, AAC, FLAC, WEBM, M4A)."
                .to_string(),
            "If the source is video, extract an audio track first, then retry listenContent on the extracted file."
                .to_string(),
        ]
    }
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

/// Resolve a local path against the session workspace.
///
/// Relative paths join `workspace_dir`. Absolute Docker workdir paths (e.g. `/app/…`)
/// are remapped to the host workspace when the session is Docker-isolated.
async fn resolve_media_local_path(
    path: &Path,
    workspace_dir: &Path,
    session_id: &str,
) -> Result<std::path::PathBuf, String> {
    let path_str = path.to_string_lossy();
    if let Some(mapped) =
        crate::session_isolation::map_docker_container_file_tool_path(session_id, &path_str).await?
    {
        return Ok(mapped);
    }
    Ok(resolve_local_path(path, workspace_dir))
}

/// Resolve, (for attach mode) sync, and workspace-bound a local media file path.
async fn prepare_local_media_path(
    path: &Path,
    workspace_dir: &Path,
    session_id: &str,
) -> Result<std::path::PathBuf, String> {
    let resolved = resolve_media_local_path(path, workspace_dir, session_id).await?;
    // Pull before canonicalize so attach-mode files that exist only in the
    // container become visible on the host staging workspace.
    if let Some(session) = crate::services::container_attach_fs::load_session(session_id).await? {
        crate::services::container_attach_fs::pull_container_file_to_host(&session, &resolved)
            .await?;
    }
    ensure_within_workspace(&resolved, workspace_dir)?;
    Ok(resolved)
}

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

fn media_local_path_error_category(error: &str) -> ErrorCategory {
    if error.contains(crate::session_isolation::OUTSIDE_DOCKER_WORKDIR_FILE_TOOL_MARKER)
        || error.contains("outside the session workspace")
    {
        ErrorCategory::PermissionDenied
    } else if error.contains("Cannot resolve path") || error.contains("Failed to read file") {
        ErrorCategory::ResourceNotFound
    } else {
        ErrorCategory::OperationFailed
    }
}

/// Map/sync a workspace-local media path and read its bytes.
async fn read_workspace_media_bytes(
    raw_path: &Path,
    workspace_dir: &Path,
    session_id: &str,
) -> Result<Vec<u8>, MCPResult> {
    let resolved = match prepare_local_media_path(raw_path, workspace_dir, session_id).await {
        Ok(path) => path,
        Err(e) => {
            return Err(
                guided_error(media_local_path_error_category(&e), e, ToolGroup::Media)
                    .to_mcp_result(),
            );
        }
    };
    match read_local_bytes(&resolved).await {
        Ok(data) => Ok(data),
        Err(e) => {
            Err(guided_error(ErrorCategory::ResourceNotFound, e, ToolGroup::Media).to_mcp_result())
        }
    }
}

/// Handle the `seeContent` tool.
pub async fn handle_see_content(
    args: Value,
    workspace_dir: std::path::PathBuf,
    session_id: String,
    see_content_successes: &AtomicUsize,
) -> Result<MCPResult, String> {
    let url_str = match args.get("url").and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => return Ok(missing_param_error("url", ToolGroup::Media)),
    };

    let slot = match SeeContentSlot::try_reserve(see_content_successes) {
        Ok(slot) => slot,
        Err(_) => return Ok(see_content_cap_exceeded_result()),
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
                match read_workspace_media_bytes(&raw_path, &workspace_dir, &session_id).await {
                    Ok(data) => (data, None),
                    Err(result) => return Ok(result),
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
    let remaining = MAX_SEE_CONTENT_SUCCESSES_PER_SESSION
        .saturating_sub(see_content_successes.load(Ordering::SeqCst));

    slot.commit();

    Ok(MCPResult {
        content: Some(vec![
            MCPContent::Text {
                text: format!(
                    "✓ Image loaded ({size_kb} KB, {mime_type})\n\nSource: {url_str}\n\n\
                     seeContent session budget: {remaining} successful load(s) remaining \
                     (max {MAX_SEE_CONTENT_SUCCESSES_PER_SESSION}). Sample sparsely; \
                     for speech-in-video prefer media__listenContent on extracted audio."
                ),
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
    session_id: String,
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
                match read_workspace_media_bytes(&raw_path, &workspace_dir, &session_id).await {
                    Ok(data) => (data, None),
                    Err(result) => return Ok(result),
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
                     (MP3, WAV, OGG, AAC, FLAC, WEBM, M4A)."
                ),
                ToolGroup::Media,
            )
            .with_guidance(listen_content_recovery(&url_str))
            .to_mcp_result());
        }
    };

    if !mime_type.starts_with("audio/") {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!("URL does not point to an audio file (detected MIME type: {mime_type})."),
            ToolGroup::Media,
        )
        .with_guidance(listen_content_recovery(&url_str))
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

// ── Handler: assistPluginStatus / deployAssistPlugin ─────────────────────────

async fn host_plugin_blocked_for_session(session_id: &str) -> Option<String> {
    if crate::mcp::builtin::workspace::utils::is_session_docker_isolated(session_id).await {
        return Some(
            "MediaAssist host plugins are disabled for Docker/Harbor sessions to prevent container breakout. Use Host isolation, or convert media inside the container with ffmpeg/CLI tools.".to_string(),
        );
    }
    None
}

pub async fn handle_assist_plugin_status(
    base_data_dir: &std::path::Path,
    session_id: &str,
) -> Result<MCPResult, String> {
    if let Some(blocked) = host_plugin_blocked_for_session(session_id).await {
        return Ok(MCPResult {
            content: Some(vec![MCPContent::Text { text: blocked.clone() }]),
            structured_content: Some(serde_json::json!({
                "installed": false,
                "hostExecutionAllowed": false,
                "error": "docker_isolation_blocked",
                "message": blocked,
            })),
            is_error: Some(false),
        });
    }

    let status = crate::media_assist::load_status(base_data_dir);
    let json = serde_json::to_value(&status).map_err(|e| e.to_string())?;
    let text = if status.installed {
        format!(
            "✓ MediaAssist plugin installed at {}\nmodalities={:?}\ntimeoutMs={}",
            status.path, status.modalities, status.timeout_ms
        )
    } else {
        let mut text = format!(
            "MediaAssist plugin not installed at {}.\nLoad @skill:libragent-plugin to implement, verify, and media__deployAssistPlugin.",
            status.path
        );
        if let Some(error) = status.error.as_ref().filter(|e| !e.is_empty()) {
            text.push_str(&format!("\nDiagnostic: {error}"));
        }
        text
    };
    Ok(MCPResult {
        content: Some(vec![MCPContent::Text { text }]),
        structured_content: Some(json),
        is_error: Some(false),
    })
}

pub async fn handle_deploy_assist_plugin(
    args: Value,
    base_data_dir: &std::path::Path,
    session_id: &str,
) -> Result<MCPResult, String> {
    if let Some(blocked) = host_plugin_blocked_for_session(session_id).await {
        return Ok(guided_error(
            ErrorCategory::PermissionDenied,
            blocked,
            ToolGroup::Media,
        )
        .with_guidance(vec![
            "MediaAssist plugins run on the host and are blocked under Docker/Harbor isolation."
                .to_string(),
            "Switch the session to Host isolation to deploy, or keep conversion inside the container."
                .to_string(),
        ])
        .to_mcp_result());
    }

    let Some(files_val) = args.get("files") else {
        return Ok(missing_param_error("files", ToolGroup::Media));
    };
    let files: Vec<crate::media_assist::DeployFile> = match serde_json::from_value(files_val.clone())
    {
        Ok(files) => files,
        Err(error) => {
            return Ok(guided_error(
                ErrorCategory::InvalidInput,
                format!("invalid files: {error}"),
                ToolGroup::Media,
            )
            .with_guidance(vec![
                "files must be an array of { path, content, base64? } objects.".to_string(),
                "Include at least manifest.json and run (or run.cmd on Windows).".to_string(),
            ])
            .to_mcp_result());
        }
    };
    match crate::media_assist::deploy_files(base_data_dir, &files) {
        Ok(status) => {
            let json = serde_json::to_value(&status).map_err(|e| e.to_string())?;
            Ok(MCPResult {
                content: Some(vec![MCPContent::Text {
                    text: format!(
                        "✓ MediaAssist plugin deployed to {}\nmodalities={:?}",
                        status.path, status.modalities
                    ),
                }]),
                structured_content: Some(json),
                is_error: Some(false),
            })
        }
        Err(error) => Ok(guided_error(
            ErrorCategory::InvalidInput,
            error,
            ToolGroup::Media,
        )
        .with_guidance(vec![
            "Include both manifest.json (interfaceVersion=1) and an executable run script.".to_string(),
            "Allowed paths: manifest.json, run (or run.exe/run.cmd/run.bat on Windows), README.md, fixtures/* only.".to_string(),
            "Follow @skill:libragent-plugin verify before deploy. Deploy requires hard user approval (not YOLO-bypassable).".to_string(),
        ])
        .to_mcp_result()),
    }
}

// ── Handler: captureScreen ───────────────────────────────────────────────────

fn screen_capture_guidance() -> Vec<String> {
    vec![
        "Verify that 'display_index' corresponds to an active monitor (use 0 for primary display).".to_string(),
        "If capturing a region, ensure 'x', 'y', 'width', and 'height' are all specified and within the captured monitor image bounds (image-local to that monitor).".to_string(),
        "Check operating system screen recording permissions (e.g. macOS System Settings > Privacy & Security, or Wayland compositor permissions).".to_string(),
        "Omit region coordinates to capture the entire display.".to_string(),
        "Click with desktop__computerControl using the same display_index and raw image-pixel x/y from this screenshot (do not divide by DPI/scale_factor). Pass width_scale/height_scale from the capture when present — omitted scales default to 1.0.".to_string(),
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
                    "Display {display_index} image ({full_w}x{full_h}); absolute origin ({monitor_x}, {monitor_y}); monitor size {monitor_width}x{monitor_height}"
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
                             display_index={display_index}, the image-pixel x/y (0,0 = top-left of this image), \
                             width_scale={width_scale}, height_scale={height_scale}. \
                             Do not divide coordinates by scale_factor/DPI — pass raw image pixels. \
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_isolation::PathMappingLayer;
    use std::path::PathBuf;

    #[test]
    fn relative_local_path_joins_workspace() {
        let workspace = PathBuf::from("/tmp/ws");
        let resolved = resolve_local_path(Path::new("image.gif"), &workspace);
        assert_eq!(resolved, workspace.join("image.gif"));
    }

    #[test]
    fn docker_workdir_absolute_maps_like_workspace_tools() {
        let host = PathBuf::from("/tmp/staging");
        let mapper = PathMappingLayer::with_container_root(host.clone(), "/app");
        assert_eq!(
            mapper.container_to_host("/app/image.gif"),
            Some(host.join("image.gif"))
        );
        assert_eq!(mapper.container_to_host("/logs/artifacts/x"), None);
    }

    #[test]
    fn outside_workdir_error_is_permission_denied() {
        let err = format!(
            "Docker container path '/logs/x' is outside /app. Shell commands may access it, but {} /app paths to the host workspace.",
            crate::session_isolation::OUTSIDE_DOCKER_WORKDIR_FILE_TOOL_MARKER
        );
        assert_eq!(
            media_local_path_error_category(&err),
            ErrorCategory::PermissionDenied
        );
    }

    #[test]
    fn looks_like_video_path_detects_common_containers() {
        assert!(looks_like_video_path("/app/video.mp4"));
        assert!(looks_like_video_path("clip.MKV?x=1"));
        assert!(!looks_like_video_path("/app/audio.wav"));
        assert!(!looks_like_video_path("/app/clip.webm")); // audio-capable for listenContent
    }

    #[test]
    fn listen_content_recovery_for_video_mentions_extract_not_apt() {
        let tips = listen_content_recovery("/app/video.mp4");
        let joined = tips.join("\n");
        assert!(joined.contains("ffmpeg"));
        assert!(joined.contains("listenContent"));
        assert!(joined.contains("seeContent"));
        assert!(joined.to_lowercase().contains("do not apt-install"));
    }

    #[test]
    fn see_content_slot_allows_up_to_max_then_rejects() {
        let counter = AtomicUsize::new(0);
        let mut committed = Vec::new();
        for _ in 0..MAX_SEE_CONTENT_SUCCESSES_PER_SESSION {
            let slot = SeeContentSlot::try_reserve(&counter).expect("slot available");
            slot.commit();
            committed.push(());
        }
        assert_eq!(
            counter.load(Ordering::SeqCst),
            MAX_SEE_CONTENT_SUCCESSES_PER_SESSION
        );
        assert!(SeeContentSlot::try_reserve(&counter).is_err());
        assert_eq!(
            counter.load(Ordering::SeqCst),
            MAX_SEE_CONTENT_SUCCESSES_PER_SESSION
        );
        let _ = committed;
    }

    #[test]
    fn see_content_slot_releases_on_drop_without_commit() {
        let counter = AtomicUsize::new(0);
        {
            let _slot = SeeContentSlot::try_reserve(&counter).expect("slot available");
            assert_eq!(counter.load(Ordering::SeqCst), 1);
        }
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        let slot = SeeContentSlot::try_reserve(&counter).expect("slot available again");
        slot.commit();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn see_content_cap_recovery_mentions_listen_and_sample() {
        let joined = see_content_cap_recovery().join("\n");
        assert!(joined.contains(&MAX_SEE_CONTENT_SUCCESSES_PER_SESSION.to_string()));
        assert!(joined.contains("listenContent"));
        assert!(joined.to_lowercase().contains("sample"));
    }

    #[test]
    fn see_tool_description_mentions_hard_session_limit() {
        let tools = super::super::tools::all_tools();
        let see = tools
            .iter()
            .find(|t| t.name == "seeContent")
            .expect("seeContent tool");
        assert!(see
            .description
            .contains(&MAX_SEE_CONTENT_SUCCESSES_PER_SESSION.to_string()));
        assert!(see.description.contains("Hard session limit"));
    }
}
