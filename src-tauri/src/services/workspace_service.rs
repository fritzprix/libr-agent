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
    /// Lists files and directories in the current session's workspace.
    pub async fn list_files(
        path: Option<String>,
        session_id: Option<String>,
    ) -> Result<Vec<WorkspaceFileItem>, String> {
        // Get the workspace base directory from session manager
        let session_manager =
            get_session_manager().map_err(|e| format!("Session manager error: {e}"))?;
        let base_dir = session_manager
            .get_session_workspace_dir_by_id(&session_id.unwrap_or_else(|| "default".to_string()));

        // Default to current directory if no path provided
        let target_path = path.unwrap_or_else(|| ".".to_string());

        // Resolve and validate path securely
        let full_path = crate::utils::security::resolve_secure_path(&base_dir, &target_path)
            .await
            .map_err(|e| format!("Invalid path: {}", e))?;

        // Read directory entries
        let mut entries = fs::read_dir(&full_path).await.map_err(|e| {
            format!(
                "Failed to read directory '{}': {}",
                full_path.display(),
                e
            )
        })?;

        let mut items = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("Failed to read directory entry: {e}"))?
        {
            let metadata = entry
                .metadata()
                .await
                .map_err(|e| format!("Failed to read metadata: {e}"))?;

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
                format!("{target_path}/{name}").replace("//", "/")
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
        // Get workspace directory via SessionManager
        let session_manager = get_session_manager().map_err(|e| e.to_string())?;
        let workspace_dir = session_manager
            .get_session_workspace_dir_by_id(&session_id.unwrap_or_else(|| "default".to_string()));

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

        // Ensure session workspace exists in pool (triggers lazy loading)
        let _workspace_path = session_manager.get_session_workspace_dir_by_id(session_id);

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

        // Ensure session workspace exists in pool (triggers lazy loading)
        let _workspace_path = session_manager.get_session_workspace_dir_by_id(session_id);

        session_manager
            .set_workspace_override(session_id, override_path)
            .await
    }

    /// Cancels the workspace override for a session.
    pub async fn cancel_override(session_id: &str) -> Result<(), String> {
        let session_manager = get_session_manager().map_err(|e| e.to_string())?;

        // Ensure session workspace exists in pool (triggers lazy loading)
        let _workspace_path = session_manager.get_session_workspace_dir_by_id(session_id);

        session_manager.remove_workspace_override(session_id).await
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
}
