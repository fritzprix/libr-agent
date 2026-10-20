//! Workspace-scoped media persistence for tool results.
//!
//! Builtin tools write bytes once under `.libragent/tool-results/media/` and return a
//! `file://` URI. `externalize_media_content_for_storage` reuses that URI when valid,
//! so download never re-ships base64 and we avoid a second on-disk copy.

use url::Url;
use uuid::Uuid;

/// Same directory used by `agent::tools::media` for externalized tool-result media.
const TOOL_RESULT_MEDIA_DIR: &str = ".libragent/tool-results/media";

/// Writes media bytes to a workspace-scoped file and returns a `file://` URI.
///
/// Filenames use a UUID only (no raw `session_id`) to avoid path separators in names.
/// Cleanup follows session workspace / `.libragent` removal.
pub fn persist_media_to_temp_file(
    workspace_dir: &std::path::Path,
    _session_id: &str,
    bytes: &[u8],
    extension: &str,
) -> Result<String, String> {
    let media_dir = workspace_dir.join(TOOL_RESULT_MEDIA_DIR);
    let extension = sanitize_extension(extension);

    std::fs::create_dir_all(&media_dir)
        .map_err(|e| format!("Failed to create media directory: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&media_dir)
            .map_err(|e| format!("Failed to read media directory metadata: {e}"))?
            .permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(&media_dir, perms)
            .map_err(|e| format!("Failed to set media directory permissions: {e}"))?;
    }

    let file_name = format!("{}.{}", Uuid::new_v4(), extension);
    let temp_path = media_dir.join(file_name);

    // Write with owner-only permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&temp_path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, bytes))
            .map_err(|e| format!("Failed to write temp media file: {e}"))?;
    }

    #[cfg(not(unix))]
    {
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, bytes))
            .map_err(|e| format!("Failed to write temp media file: {e}"))?;
    }

    let file_url = Url::from_file_path(&temp_path).map_err(|_| {
        format!(
            "Failed to convert path to file URL: {}",
            temp_path.display()
        )
    })?;

    log::info!(
        "Persisted media to workspace file: {} ({} bytes)",
        temp_path.display(),
        bytes.len()
    );

    Ok(file_url.to_string())
}

fn sanitize_extension(extension: &str) -> &str {
    if !extension.is_empty() && extension.chars().all(|c| c.is_ascii_alphanumeric()) {
        extension
    } else {
        "bin"
    }
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
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/ogg" => "ogg",
        "audio/webm" => "webm",
        "audio/aac" => "aac",
        "audio/flac" => "flac",
        _ => "bin",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_sanitize_extension_rejects_path_separators() {
        assert_eq!(sanitize_extension("../x"), "bin");
        assert_eq!(sanitize_extension("png"), "png");
    }

    #[test]
    fn test_persist_media_creates_file_in_tool_results_dir() {
        let temp_workspace = tempfile::tempdir().unwrap();
        let workspace_path = temp_workspace.path();
        let test_data = b"test image data";

        let result = persist_media_to_temp_file(workspace_path, "test-session", test_data, "png");

        assert!(result.is_ok(), "Should create temp file successfully");
        let file_url = result.unwrap();
        assert!(file_url.starts_with("file://"), "Should return file:// URL");

        let media_dir = workspace_path.join(TOOL_RESULT_MEDIA_DIR);
        assert!(
            media_dir.exists(),
            "Should create tool-results media directory"
        );
        assert!(media_dir.is_dir(), "media dir should be a directory");

        let files: Vec<_> = std::fs::read_dir(&media_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files.len(), 1, "Should create exactly one file");

        let file_path = files[0].path();
        assert!(file_path.exists(), "File should exist");

        let content = std::fs::read(&file_path).unwrap();
        assert_eq!(content, test_data, "File content should match");

        let file_name = file_path.file_name().unwrap().to_str().unwrap();
        assert!(file_name.ends_with(".png"), "Should have correct extension");
        assert!(
            !file_name.contains("test-session"),
            "Filename should not embed session id"
        );
    }

    #[test]
    fn test_persist_media_uses_uuid_not_timestamp() {
        let temp_workspace = tempfile::tempdir().unwrap();
        let workspace_path = temp_workspace.path();
        let test_data = b"test";

        let result1 = persist_media_to_temp_file(workspace_path, "s1", test_data, "png");
        let result2 = persist_media_to_temp_file(workspace_path, "s1", test_data, "png");

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        assert_ne!(
            result1.unwrap(),
            result2.unwrap(),
            "Should generate unique filenames"
        );

        let media_dir = workspace_path.join(TOOL_RESULT_MEDIA_DIR);
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

        let media_dir = workspace_path.join(TOOL_RESULT_MEDIA_DIR);
        let dir_mode = std::fs::metadata(&media_dir).unwrap().permissions().mode();
        assert_eq!(dir_mode & 0o777, 0o700, "Directory should be 0700");

        let file_path = std::fs::read_dir(&media_dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();

        let mode = std::fs::metadata(&file_path).unwrap().permissions().mode();
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

        let canonical_workspace = std::fs::canonicalize(workspace_path).unwrap();
        let canonical_file = std::fs::canonicalize(&file_path).unwrap();

        assert!(
            canonical_file.starts_with(&canonical_workspace),
            "Temp file should be within workspace directory"
        );
    }
}
