//! Temporary file persistence for media content.
//!
//! This module provides utilities to persist media bytes to workspace-scoped temp files
//! and return file:// URIs, avoiding large base64 payloads over Tauri IPC.

use std::path::PathBuf;
use url::Url;
use uuid::Uuid;

/// Writes media bytes to a workspace-scoped temporary file and returns a file:// URI.
///
/// Files are created under `<workspace>/.libr-media/` to pass workspace scope validation.
/// Cleanup happens when the session directory is deleted or via TTL policy.
pub fn persist_media_to_temp_file(
    workspace_dir: &std::path::Path,
    session_id: &str,
    bytes: &[u8],
    extension: &str,
) -> Result<String, String> {
    let media_dir = workspace_dir.join(".libr-media");

    // Create .libr-media directory if it doesn't exist
    std::fs::create_dir_all(&media_dir)
        .map_err(|e| format!("Failed to create media directory: {e}"))?;

    // Use UUID to avoid timestamp collisions
    let uuid = Uuid::new_v4();
    let file_name = format!("{}-{}.{}", session_id, uuid, extension);
    let temp_path = media_dir.join(file_name);

    // Write with owner-only permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o600)
            .open(&temp_path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, bytes))
            .map_err(|e| format!("Failed to write temp media file: {e}"))?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&temp_path, bytes)
            .map_err(|e| format!("Failed to write temp media file: {e}"))?;
    }

    let file_url = Url::from_file_path(&temp_path).map_err(|_| {
        format!(
            "Failed to convert path to file URL: {}",
            temp_path.display()
        )
    })?;

    log::info!(
        "Persisted media to workspace temp file: {} ({} bytes)",
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_extension_from_mime_images() {
        assert_eq!(extension_from_mime("image/png"), "png");
        assert_eq!(extension_from_mime("image/jpeg"), "jpg");
        assert_eq!(extension_from_mime("image/jpg"), "jpg");
        assert_eq!(extension_from_mime("image/webp"), "webp");
        assert_eq!(extension_from_mime("image/gif"), "gif");
        assert_eq!(extension_from_mime("image/svg+xml"), "svg");
    }

    #[test]
    fn test_extension_from_mime_audio() {
        assert_eq!(extension_from_mime("audio/wav"), "wav");
        assert_eq!(extension_from_mime("audio/x-wav"), "wav");
        assert_eq!(extension_from_mime("audio/mpeg"), "mp3");
        assert_eq!(extension_from_mime("audio/mp3"), "mp3");
    }

    #[test]
    fn test_extension_from_mime_fallback() {
        assert_eq!(extension_from_mime("unknown/type"), "bin");
        assert_eq!(extension_from_mime(""), "bin");
    }

    #[test]
    fn test_persist_media_creates_file_in_workspace() {
        let temp_workspace = tempfile::tempdir().unwrap();
        let workspace_path = temp_workspace.path();
        let test_data = b"test image data";

        let result = persist_media_to_temp_file(workspace_path, "test-session", test_data, "png");

        assert!(result.is_ok(), "Should create temp file successfully");
        let file_url = result.unwrap();
        assert!(file_url.starts_with("file://"), "Should return file:// URL");

        // Verify file was created in .libr-media subdirectory
        let media_dir = workspace_path.join(".libr-media");
        assert!(media_dir.exists(), "Should create .libr-media directory");
        assert!(media_dir.is_dir(), ".libr-media should be a directory");

        // Verify file exists and has correct content
        let files: Vec<_> = std::fs::read_dir(&media_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files.len(), 1, "Should create exactly one file");

        let file_path = files[0].path();
        assert!(file_path.exists(), "File should exist");

        let content = std::fs::read(&file_path).unwrap();
        assert_eq!(content, test_data, "File content should match");

        // Verify filename format
        let file_name = file_path.file_name().unwrap().to_str().unwrap();
        assert!(
            file_name.starts_with("test-session-"),
            "Should include session ID"
        );
        assert!(file_name.ends_with(".png"), "Should have correct extension");
    }

    #[test]
    fn test_persist_media_uses_uuid_not_timestamp() {
        let temp_workspace = tempfile::tempdir().unwrap();
        let workspace_path = temp_workspace.path();
        let test_data = b"test";

        // Create two files quickly in succession
        let result1 = persist_media_to_temp_file(workspace_path, "s1", test_data, "png");
        let result2 = persist_media_to_temp_file(workspace_path, "s1", test_data, "png");

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        // Both should succeed with different paths (UUID prevents collision)
        assert_ne!(
            result1.unwrap(),
            result2.unwrap(),
            "Should generate unique filenames"
        );

        // Verify both files exist
        let media_dir = workspace_path.join(".libr-media");
        let file_count = std::fs::read_dir(&media_dir).unwrap().count();
        assert_eq!(file_count, 2, "Should create two distinct files");
    }

    #[cfg(unix)]
    #[test]
    fn test_persist_media_sets_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_workspace = tempfile::tempdir().unwrap();
        let workspace_path = temp_workspace.path();
        let test_data = b"sensitive data";

        let result = persist_media_to_temp_file(workspace_path, "s1", test_data, "png");
        assert!(result.is_ok());

        // Find the created file
        let media_dir = workspace_path.join(".libr-media");
        let file_path = std::fs::read_dir(&media_dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();

        let metadata = std::fs::metadata(&file_path).unwrap();
        let mode = metadata.permissions().mode();

        // Check that file has 0o600 permissions (owner read+write only)
        assert_eq!(
            mode & 0o777,
            0o600,
            "File should have owner-only permissions (0600)"
        );
    }

    #[test]
    fn test_persist_media_path_within_workspace() {
        let temp_workspace = tempfile::tempdir().unwrap();
        let workspace_path = temp_workspace.path();
        let test_data = b"test";

        let result = persist_media_to_temp_file(workspace_path, "s1", test_data, "png");
        assert!(result.is_ok());

        let file_url = result.unwrap();
        let file_url_parsed = Url::parse(&file_url).unwrap();
        let file_path = file_url_parsed.to_file_path().unwrap();

        // Verify the file path is within the workspace
        let canonical_workspace = std::fs::canonicalize(workspace_path).unwrap();
        let canonical_file = std::fs::canonicalize(&file_path).unwrap();

        assert!(
            canonical_file.starts_with(&canonical_workspace),
            "Temp file should be within workspace directory"
        );
    }
}
