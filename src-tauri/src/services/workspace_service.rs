use crate::repositories::session_repository::SessionRepository;
use crate::session::get_session_manager;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Represents a file or directory item in the workspace for display in the frontend.
#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileItem {
    /// The name of the file or directory.
    pub name: String,
    /// True if the item is a directory.
    pub is_directory: bool,
    /// The relative path of the item within the workspace.
    pub path: String,
    /// The size of the file in bytes, or `None` for a directory.
    pub size: Option<u64>,
    /// The last modified timestamp as a formatted string, or `None`.
    pub modified: Option<String>,
}

/// File content payload returned for in-app workspace preview.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileContentResponse {
    pub content: String,
    pub is_binary: bool,
    pub size: u64,
    pub mime_type: String,
}

fn get_preview_mime_type(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "yaml" | "yml" => "application/yaml",
        "toml" => "application/toml",
        "xml" => "application/xml",
        "md" | "markdown" => "text/markdown",
        "ts" | "tsx" => "text/typescript",
        "js" | "jsx" => "text/javascript",
        "css" | "scss" => "text/css",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "txt" | "log" => "text/plain",
        _ => "application/octet-stream",
    }
}

pub struct WorkspaceService;

impl WorkspaceService {
    async fn sync_active_session_workspace_override(
        session_id: &str,
        workspace_override: Option<String>,
    ) {
        let Some(active_sessions) = crate::state::try_get_active_sessions() else {
            return;
        };

        let cached_stable_prompt = {
            let mut active = active_sessions.write().await;
            let Some(session) = active.get_mut(session_id) else {
                return;
            };

            session.metadata.workspace_override = workspace_override;
            session.cached_stable_prompt.clone()
        };

        *cached_stable_prompt.write().await = None;
    }

    /// Lists files and directories in the current session's workspace.
    pub async fn list_files(
        path: Option<String>,
        session_id: Option<String>,
    ) -> Result<Vec<WorkspaceFileItem>, String> {
        let session_manager =
            get_session_manager().map_err(|e| format!("Session manager error: {e}"))?;
        let session_id = session_id.unwrap_or_else(|| "default".to_string());
        let base_dir =
            crate::session::resolve_session_workspace_dir(session_manager, &session_id).await?;

        // Default to current directory if no path provided
        let target_path = path.unwrap_or_else(|| ".".to_string());

        // Resolve and validate path securely
        let full_path = crate::utils::security::resolve_secure_path(&base_dir, &target_path)
            .await
            .map_err(|e| format!("Invalid path: {}", e))?;

        // Read directory entries
        let mut entries = fs::read_dir(&full_path)
            .await
            .map_err(|e| format!("Failed to read directory '{}': {}", full_path.display(), e))?;

        let mut items = Vec::new();

        loop {
            let entry = match entries.next_entry().await {
                Ok(Some(e)) => e,
                Ok(None) => break,
                Err(e) => {
                    log::warn!("Skipping unreadable directory entry: {e}");
                    continue;
                }
            };

            let metadata = match entry.metadata().await {
                Ok(m) => m,
                Err(e) => {
                    log::warn!(
                        "Skipping '{}': failed to read metadata: {e}",
                        entry.file_name().to_string_lossy()
                    );
                    continue;
                }
            };

            let name = entry.file_name().to_string_lossy().to_string();
            let is_directory = metadata.is_dir();
            let size = if is_directory {
                None
            } else {
                Some(metadata.len())
            };

            // Format modification time
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| {
                    let datetime = DateTime::<Utc>::from_timestamp(duration.as_secs() as i64, 0);
                    datetime
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                        .unwrap_or_else(|| "Unknown".to_string())
                });

            let relative_path = if target_path == "." {
                name.clone()
            } else {
                let p = PathBuf::from(&target_path)
                    .join(&name)
                    .to_string_lossy()
                    .to_string();
                #[cfg(target_os = "windows")]
                let p = p.replace('\\', "/");
                p
            };

            items.push(WorkspaceFileItem {
                name,
                is_directory,
                path: relative_path,
                size,
                modified,
            });
        }

        // Sort: directories first, then files, both alphabetically
        items.sort_by(|a, b| match (a.is_directory, b.is_directory) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        Ok(items)
    }

    /// Opens a workspace file with the system's default application.
    pub async fn open_file_with_default_app(
        file_path: String,
        session_id: Option<String>,
    ) -> Result<(), String> {
        let session_manager = get_session_manager().map_err(|e| e.to_string())?;
        let session_id = session_id.unwrap_or_else(|| "default".to_string());
        let workspace_dir =
            crate::session::resolve_session_workspace_dir(session_manager, &session_id).await?;

        // Resolve and validate path securely
        let full_path = crate::utils::security::resolve_secure_path(&workspace_dir, &file_path)
            .await
            .map_err(|e| format!("Access denied or file not found: {}", e))?;

        // Security validation: ensure it's a file, not a directory
        if !full_path.is_file() {
            return Err("Cannot open directories with default app".to_string());
        }

        // Convert to absolute path string
        let abs_path_str = full_path
            .to_str()
            .ok_or_else(|| "Invalid path encoding".to_string())?;

        // Use tauri-plugin-opener to open file with system default app
        tauri_plugin_opener::open_path(abs_path_str, None::<&str>)
            .map_err(|e| format!("Failed to open file: {}", e))?;

        Ok(())
    }

    /// Gets the current workspace override for a session.
    pub async fn get_override(session_id: &str) -> Result<Option<String>, String> {
        let session_manager = get_session_manager().map_err(|e| e.to_string())?;
        crate::session::hydrate_persisted_workspace_override_from_global(
            session_manager,
            session_id,
        )
        .await?;

        let info = session_manager
            .get_session_info(session_id)
            .ok_or("Session not found")?;
        Ok(info
            .workspace_override
            .map(|p| p.to_string_lossy().to_string()))
    }

    /// Sets the workspace override for a session.
    pub async fn set_override(session_id: &str, override_path: String) -> Result<(), String> {
        let override_path = PathBuf::from(&override_path);

        if !override_path.exists() {
            return Err(format!("Path does not exist: {}", override_path.display()));
        }

        if !override_path.is_dir() {
            return Err(format!(
                "Path is not a directory: {}",
                override_path.display()
            ));
        }

        if !Self::check_dir_access(&override_path).await? {
            return Err("Directory is not accessible (check permissions)".to_string());
        }

        let session_manager = get_session_manager().map_err(|e| e.to_string())?;
        crate::session::resolve_session_workspace_dir(session_manager, session_id).await?;

        // Reject non-UTF-8 paths: they cannot be round-tripped through the DB correctly
        let override_str = override_path
            .to_str()
            .ok_or_else(|| "Invalid path encoding: path contains non-UTF-8 characters".to_string())?
            .to_string();

        // Persist to DB first — if this fails, the in-memory state is left unchanged,
        // so runtime and persisted state stay consistent.
        let session_repo = crate::state::get_session_repository();
        session_repo
            .update_workspace_override(session_id, Some(override_str.clone()))
            .await
            .map_err(|e| format!("Failed to persist workspace override: {}", e))?;

        // Only update in-memory pool after DB write succeeds
        session_manager
            .set_workspace_override(session_id, override_path)
            .await?;

        Self::sync_active_session_workspace_override(session_id, Some(override_str)).await;
        crate::agent::tauri_events::emit_resource_updated(
            "session",
            "update",
            Some(session_id.to_string()),
        );

        Ok(())
    }

    /// Prepare the app-local teamwork artifact directory for a governing/root session.
    pub async fn provision_teamwork_workspace(session_id: &str) -> Result<String, String> {
        let session_manager = get_session_manager().map_err(|e| e.to_string())?;
        let teamwork_artifact_dir =
            crate::session::prepare_teamwork_artifact_dir_for_session(session_manager, session_id)
                .await?;

        let teamwork_artifact_dir_str = teamwork_artifact_dir
            .to_str()
            .ok_or_else(|| {
                format!(
                    "Invalid teamwork artifact path encoding: {}",
                    teamwork_artifact_dir.display()
                )
            })?
            .to_string();
        Ok(teamwork_artifact_dir_str)
    }

    /// Cancels the workspace override for a session.
    pub async fn cancel_override(session_id: &str) -> Result<(), String> {
        let session_manager = get_session_manager().map_err(|e| e.to_string())?;
        crate::session::ensure_session_workspace_dir(
            crate::state::get_session_repository(),
            session_manager,
            session_id,
        )
        .await?;

        // Clear from DB first — if this fails we return early before touching in-memory state,
        // keeping both sources of truth consistent.
        let session_repo = crate::state::get_session_repository();
        session_repo
            .update_workspace_override(session_id, None)
            .await
            .map_err(|e| format!("Failed to clear workspace override: {}", e))?;

        // Only remove from in-memory pool after DB write succeeds
        session_manager
            .remove_workspace_override(session_id)
            .await?;

        Self::sync_active_session_workspace_override(session_id, None).await;
        crate::agent::tauri_events::emit_resource_updated(
            "session",
            "update",
            Some(session_id.to_string()),
        );

        Ok(())
    }

    /// Checks if a directory is accessible.
    async fn check_dir_access(path: &PathBuf) -> Result<bool, String> {
        match fs::read_dir(path).await {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    Ok(false)
                } else {
                    Err(e.to_string())
                }
            }
        }
    }

    /// A session-aware method to write a file to the current session's workspace.
    ///
    /// This ensures that file operations are contained within the active session's
    /// designated workspace directory, preventing writes to unintended locations.
    pub async fn workspace_write_file(
        file_path: &str,
        content: &[u8],
        session_id: Option<String>,
    ) -> Result<(), String> {
        let session_manager =
            get_session_manager().map_err(|e| format!("Session manager error: {e}"))?;

        // Session ID is mandatory for workspace operations in V2 logic
        if let Some(sid) = session_id {
            let workspace_dir =
                crate::session::resolve_session_workspace_dir(session_manager, &sid).await?;
            // Create a temporary secure file manager for this operation
            let manager =
                crate::services::SecureFileManager::new_scoped_with_base_dir(workspace_dir);
            return manager.write_file(file_path, content).await;
        }

        Err("Session ID is required for workspace write operations".to_string())
    }

    /// Reads a file from the session's workspace for in-app inline preview.
    ///
    /// Enforces:
    /// - Path security (must be within workspace directory)
    /// - Strict 2 MB maximum preview size
    /// - Image encoding to base64
    /// - Detection of binary / null bytes / invalid UTF-8 with fallback
    pub async fn read_file_content(
        file_path: String,
        session_id: Option<String>,
    ) -> Result<WorkspaceFileContentResponse, String> {
        let session_manager =
            get_session_manager().map_err(|e| format!("Session manager error: {e}"))?;
        let session_id = session_id.unwrap_or_else(|| "default".to_string());
        let workspace_dir =
            crate::session::resolve_session_workspace_dir(session_manager, &session_id).await?;

        // Resolve and validate path securely against traversal
        let full_path = crate::utils::security::resolve_secure_path(&workspace_dir, &file_path)
            .await
            .map_err(|e| format!("Access denied or file not found: {e}"))?;

        Self::read_file_content_from_path(&full_path).await
    }

    /// Reads and formats file content for inline preview from a resolved filesystem path.
    pub async fn read_file_content_from_path(
        full_path: &Path,
    ) -> Result<WorkspaceFileContentResponse, String> {
        const MAX_PREVIEW_SIZE: u64 = 2 * 1024 * 1024; // 2 MB

        if !full_path.is_file() {
            return Err("Target path is not a file".to_string());
        }

        let metadata = fs::metadata(full_path).await.map_err(|e| e.to_string())?;
        let size = metadata.len();
        if size > MAX_PREVIEW_SIZE {
            return Err(format!(
                "File too large for inline preview ({:.2} MB). Maximum allowed size is 2 MB.",
                size as f64 / (1024.0 * 1024.0)
            ));
        }

        let bytes = fs::read(full_path).await.map_err(|e| e.to_string())?;
        let ext = full_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        let mime_type = get_preview_mime_type(&ext).to_string();

        let is_image = matches!(
            ext.as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico"
        );

        if is_image {
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            return Ok(WorkspaceFileContentResponse {
                content: encoded,
                is_binary: true,
                size,
                mime_type,
            });
        }

        // Check for null bytes in non-image files (indicator of binary content)
        if bytes.contains(&0) {
            return Ok(WorkspaceFileContentResponse {
                content: String::new(),
                is_binary: true,
                size,
                mime_type,
            });
        }

        // Validate UTF-8 decoding
        match String::from_utf8(bytes) {
            Ok(content) => Ok(WorkspaceFileContentResponse {
                content,
                is_binary: false,
                size,
                mime_type,
            }),
            Err(_) => Ok(WorkspaceFileContentResponse {
                content: String::new(),
                is_binary: true,
                size,
                mime_type,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_file_content_response_serde_camel_case() {
        let resp = WorkspaceFileContentResponse {
            content: "console.log('hello');".to_string(),
            is_binary: false,
            size: 22,
            mime_type: "text/javascript".to_string(),
        };

        let json = serde_json::to_string(&resp).expect("Serialization failed");
        assert!(json.contains("\"isBinary\":false"));
        assert!(json.contains("\"mimeType\":\"text/javascript\""));
        assert!(json.contains("\"size\":22"));
        assert!(json.contains("\"content\":\"console.log('hello');\""));

        let deserialized: WorkspaceFileContentResponse =
            serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(deserialized, resp);
    }

    #[test]
    fn test_get_preview_mime_type() {
        assert_eq!(get_preview_mime_type("md"), "text/markdown");
        assert_eq!(get_preview_mime_type("html"), "text/html");
        assert_eq!(get_preview_mime_type("ts"), "text/typescript");
        assert_eq!(get_preview_mime_type("png"), "image/png");
        assert_eq!(get_preview_mime_type("csv"), "text/csv");
        assert_eq!(get_preview_mime_type("unknown"), "application/octet-stream");
    }

    #[tokio::test]
    async fn test_read_file_content_from_path() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");

        // 1. Text file
        let text_path = temp_dir.path().join("test.txt");
        tokio::fs::write(&text_path, "Hello, world!").await.unwrap();
        let resp = WorkspaceService::read_file_content_from_path(&text_path)
            .await
            .expect("Failed to read text file");
        assert_eq!(resp.content, "Hello, world!");
        assert!(!resp.is_binary);
        assert_eq!(resp.mime_type, "text/plain");
        assert_eq!(resp.size, 13);

        // 2. Binary file with null byte
        let bin_path = temp_dir.path().join("binary.dat");
        tokio::fs::write(&bin_path, b"abc\x00def").await.unwrap();
        let resp = WorkspaceService::read_file_content_from_path(&bin_path)
            .await
            .expect("Failed to read binary file");
        assert_eq!(resp.content, "");
        assert!(resp.is_binary);
        assert_eq!(resp.mime_type, "application/octet-stream");

        // 3. Image file
        let img_path = temp_dir.path().join("image.png");
        tokio::fs::write(&img_path, b"fake png bytes").await.unwrap();
        let resp = WorkspaceService::read_file_content_from_path(&img_path)
            .await
            .expect("Failed to read image file");
        assert!(resp.is_binary);
        assert_eq!(resp.mime_type, "image/png");
        assert!(!resp.content.is_empty());

        // 4. Over 2MB file error
        let large_path = temp_dir.path().join("large.txt");
        let large_file = tokio::fs::File::create(&large_path).await.unwrap();
        large_file.set_len(2 * 1024 * 1024 + 1).await.unwrap();
        let err = WorkspaceService::read_file_content_from_path(&large_path)
            .await
            .expect_err("Should reject file > 2MB");
        assert!(err.contains("File too large"));
    }
}
