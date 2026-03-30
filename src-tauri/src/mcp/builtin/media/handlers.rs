use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use std::path::Path;
use url::Url;

use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, ToolGroup,
};
use crate::mcp::types::{MCPContent, MCPResult};

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
        Some(_) => return Ok(missing_param_error("url", ToolGroup::Workspace)),
        None => return Ok(missing_param_error("url", ToolGroup::Workspace)),
    };

    let (bytes, content_type_header) = match parse_source(&url_str)? {
        ContentSource::Http(url) => fetch_http_bytes(&url).await?,
        ContentSource::LocalFile(raw_path) => {
            let resolved = resolve_local_path(&raw_path, &workspace_dir);
            ensure_within_workspace(&resolved, &workspace_dir)?;
            let data = read_local_bytes(&resolved).await?;
            (data, None)
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
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }
    };

    if !mime_type.starts_with("image/") {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!("URL does not point to an image (detected MIME type: {mime_type})."),
            ToolGroup::Workspace,
        )
        .to_mcp_result());
    }

    let data = general_purpose::STANDARD.encode(&bytes);
    let size_kb = bytes.len() / 1024;

    Ok(MCPResult {
        content: Some(vec![
            MCPContent::Text {
                text: format!("Image loaded: {url_str}\nType: {mime_type} | Size: {size_kb} KB"),
                is_error: None,
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
        Some(_) => return Ok(missing_param_error("url", ToolGroup::Workspace)),
        None => return Ok(missing_param_error("url", ToolGroup::Workspace)),
    };

    let (bytes, content_type_header) = match parse_source(&url_str)? {
        ContentSource::Http(url) => fetch_http_bytes(&url).await?,
        ContentSource::LocalFile(raw_path) => {
            let resolved = resolve_local_path(&raw_path, &workspace_dir);
            ensure_within_workspace(&resolved, &workspace_dir)?;
            let data = read_local_bytes(&resolved).await?;
            (data, None)
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
                ToolGroup::Workspace,
            )
            .to_mcp_result());
        }
    };

    if !mime_type.starts_with("audio/") {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!("URL does not point to an audio file (detected MIME type: {mime_type})."),
            ToolGroup::Workspace,
        )
        .to_mcp_result());
    }

    let data = general_purpose::STANDARD.encode(&bytes);
    let size_kb = bytes.len() / 1024;

    Ok(MCPResult {
        content: Some(vec![
            MCPContent::Text {
                text: format!(
                    "Audio loaded: {url_str}\nType: {mime_type} | Size: {size_kb} KB\n\n\
                     Note: Audio content is only understood by models with native audio input \
                     support (e.g., GPT-4o Audio Preview). Other models will not interpret the audio."
                ),
                is_error: None,
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
