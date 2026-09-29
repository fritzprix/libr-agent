//! MIME detection helpers for media URLs and local paths.

use std::path::Path;

/// Detect image MIME type from a file extension.
pub(super) fn image_mime_from_ext(ext: &str) -> Option<&'static str> {
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
pub(super) fn audio_mime_from_ext(ext: &str) -> Option<&'static str> {
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
pub(super) fn ext_from_url_path(url: &str) -> Option<String> {
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
pub(super) fn resolve_image_mime(url: &str, content_type_header: Option<&str>) -> Option<String> {
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
pub(super) fn resolve_audio_mime(url: &str, content_type_header: Option<&str>) -> Option<String> {
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
pub(super) fn looks_like_video_path(url: &str) -> bool {
    matches!(
        ext_from_url_path(url).as_deref(),
        Some("mp4" | "mkv" | "mov" | "avi" | "m4v" | "mpeg" | "mpg" | "wmv")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_like_video_path_detects_common_containers() {
        assert!(looks_like_video_path("/app/video.mp4"));
        assert!(looks_like_video_path("clip.MKV?x=1"));
        assert!(!looks_like_video_path("/app/audio.wav"));
        assert!(!looks_like_video_path("/app/clip.webm")); // audio-capable for listenContent
    }
}
