use crate::repositories::session_repository::SessionRepository;
use crate::session::get_session_manager;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
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
            let manager = crate::services::SecureFileManager::new_with_base_dir(workspace_dir);
            return manager.write_file(file_path, content).await;
        }

        Err("Session ID is required for workspace write operations".to_string())
    }
}
