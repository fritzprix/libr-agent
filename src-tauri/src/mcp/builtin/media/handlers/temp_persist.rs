//! Temporary file persistence for media content.
//!
//! This module provides utilities to persist media bytes to temporary files
//! and return file:// URIs, avoiding large base64 payloads over Tauri IPC.

use std::path::PathBuf;
use url::Url;

/// Writes media bytes to a temporary file and returns a file:// URI.
///
/// The temp file is created in the system temp directory with a session-scoped name.
/// Cleanup happens naturally on app restart or system temp cleanup.
pub fn persist_media_to_temp_file(
    session_id: &str,
    bytes: &[u8],
    extension: &str,
) -> Result<String, String> {
    let temp_dir = std::env::temp_dir();
    let timestamp = chrono::Utc::now().timestamp_millis();
    let file_name = format!("libragent-media-{}-{}.{}", session_id, timestamp, extension);
    let temp_path = temp_dir.join(file_name);

    std::fs::write(&temp_path, bytes)
        .map_err(|e| format!("Failed to write temp media file: {e}"))?;

    let file_url = Url::from_file_path(&temp_path)
        .map_err(|_| format!("Failed to convert path to file URL: {}", temp_path.display()))?;

    log::info!(
        "Persisted media to temp file: {} ({} bytes)",
        temp_path.display(),
        bytes.len()
    );

    Ok(file_url.to_string())
}

/// Extracts the file extension from a MIME type.
pub fn extension_from_mime(mime_type: &str) -> &str {
    match mime_type {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        "image/svg+xml" => "svg",
        "image/tiff" => "tiff",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/ogg" => "ogg",
        "audio/webm" => "webm",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        _ => "bin",
    }
}
