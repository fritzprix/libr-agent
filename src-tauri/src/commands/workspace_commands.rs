/// Workspace-related Tauri commands
///
/// This module contains commands for workspace and application directory management,
/// including file listing, data directories, and log directories.
use crate::session::get_session_manager;
use chrono::{DateTime, Utc};
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

/// A simple command to test the frontend-backend connection.
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
}

/// Lists files and directories in the current session's workspace.
///
/// This command reads the contents of a specified path within the session's workspace,
/// performs security validation, and returns a structured list of items.
///
/// # Arguments
/// * `path` - An optional relative path within the workspace. Defaults to the root.
///
/// # Returns
/// A `Result` containing a vector of `WorkspaceFileItem` objects, or an error string on failure.
#[tauri::command]
pub async fn list_workspace_files(path: Option<String>) -> Result<Vec<WorkspaceFileItem>, String> {
    // Get the workspace base directory from session manager
    let session_manager =
        get_session_manager().map_err(|e| format!("Session manager error: {e}"))?;
    let base_dir = session_manager.get_session_workspace_dir();

    // Default to current directory if no path provided
    let target_path = path.unwrap_or_else(|| ".".to_string());
    let full_path = base_dir.join(&target_path);

    // Validate path is within workspace
    let canonical_base = base_dir
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize base dir: {e}"))?;
    let canonical_target = full_path
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize target path: {e}"))?;

    if !canonical_target.starts_with(&canonical_base) {
        return Err("Path is outside workspace".to_string());
    }

    // Read directory entries
    let mut entries = fs::read_dir(&full_path)
        .await
        .map_err(|e| format!("Failed to read directory '{}': {}", full_path.display(), e))?;

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

/// Gets the application's base data directory.
#[tauri::command]
pub async fn get_app_data_dir() -> Result<String, String> {
    let path = get_session_manager()?.get_base_data_dir();
    Ok(path.to_string_lossy().to_string())
}

/// Gets the application's log directory path.
#[tauri::command]
pub async fn get_app_logs_dir() -> Result<String, String> {
    let path = get_session_manager()?.get_logs_dir();
    Ok(path.to_string_lossy().to_string())
}
