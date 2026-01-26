/// Workspace-related Tauri commands
///
/// This module contains commands for workspace and application directory management,
/// including file listing, data directories, and log directories.
use crate::session::get_session_manager;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::process::Command;
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
/// * `session_id` - An optional session ID to specify which session's workspace to list.
///                  If not provided, uses the current session.
///
/// # Returns
/// A `Result` containing a vector of `WorkspaceFileItem` objects, or an error string on failure.
#[tauri::command]
pub async fn list_workspace_files(
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

/// Opens a workspace file with the system's default application.
///
/// This command resolves the file path within the workspace, performs security
/// validation, and uses the system's default application to open the file.
///
/// # Arguments
/// * `file_path` - The relative path of the file within the workspace to open.
///
/// # Returns
/// A `Result` indicating success or an error string on failure.
#[tauri::command]
pub async fn open_workspace_file_with_default_app(
    file_path: String,
    session_id: Option<String>,
) -> Result<(), String> {
    // Get workspace directory via SessionManager
    let session_manager = get_session_manager().map_err(|e| e.to_string())?;
    let workspace_dir = session_manager
        .get_session_workspace_dir_by_id(&session_id.unwrap_or_else(|| "default".to_string()));

    // Construct the full path of the requested file
    let full_path = workspace_dir.join(&file_path);

    // Security validation: verify file exists
    if !full_path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    // Security validation: ensure path is within workspace
    if !full_path.starts_with(&workspace_dir) {
        return Err("Access denied: Path outside workspace".to_string());
    }

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

#[tauri::command]
pub async fn open_workspace_in_explorer(session_id: String) -> Result<(), String> {
    let session_manager = get_session_manager().map_err(|e| e.to_string())?;
    let workspace_path = session_manager.get_session_workspace_dir_by_id(&session_id);

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(workspace_path)
            .spawn()
            .map_err(|e| format!("Failed to open Explorer: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(workspace_path)
            .spawn()
            .map_err(|e| format!("Failed to open Finder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        let file_managers = ["nautilus", "dolphin", "thunar", "pcmanfm"];
        let mut opened = false;
        let mut errors: Vec<String> = Vec::new();

        for fm in &file_managers {
            match Command::new(fm).arg(&workspace_path).spawn() {
                Ok(_) => {
                    opened = true;
                    break;
                }
                Err(e) => {
                    errors.push(format!("{}: {}", fm, e));
                }
            }
        }

        if !opened {
            let error_details = if errors.is_empty() {
                String::new()
            } else {
                format!("\n\nAttempted commands and errors:\n{}", errors.join("\n"))
            };
            return Err(format!(
                "No file manager found. Supported: nautilus, dolphin, thunar, pcmanfm{}",
                error_details
            ));
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn open_workspace_in_terminal(session_id: String) -> Result<(), String> {
    let session_manager = get_session_manager().map_err(|e| e.to_string())?;
    let workspace_path = session_manager.get_session_workspace_dir_by_id(&session_id);

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args([
                "/c",
                "start",
                "Agent Workspace",
                "/D",
                &workspace_path.to_string_lossy(),
                "cmd",
                "/k",
            ])
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        let path_str = workspace_path.to_string_lossy();
        // Quote for shell
        let shell_quoted = format!("'{}'", path_str.replace("'", "'\\''"));
        // Escape for AppleScript string
        let script_cmd = format!("cd {}", shell_quoted);
        let applescript_escaped = script_cmd.replace("\\", "\\\\").replace("\"", "\\\"");

        let script = format!(
            "tell application \"Terminal\" to do script \"{}\"",
            applescript_escaped
        );

        Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map_err(|e| format!("Failed to open Terminal: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Err(format!(
            "Terminal launch not supported on Linux. No standard command available. \
             Open a terminal manually and navigate to: {}",
            workspace_path.display()
        ))
    }

    // For Windows and macOS, the terminal was already launched above
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        Ok(())
    }

    // Fallback for unsupported platforms (unlikely, but comprehensive)
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("Terminal launch not supported on this platform".to_string())
    }
}

#[tauri::command]
pub async fn get_workspace_override(session_id: String) -> Result<Option<String>, String> {
    let session_manager = get_session_manager().map_err(|e| e.to_string())?;

    // Ensure session workspace exists in pool (triggers lazy loading)
    let _workspace_path = session_manager.get_session_workspace_dir_by_id(&session_id);

    let info = session_manager
        .get_session_info(&session_id)
        .ok_or("Session not found")?;
    Ok(info
        .workspace_override
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn set_workspace_override(
    session_id: String,
    override_path: String,
) -> Result<(), String> {
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

    if !check_dir_access(&override_path).await? {
        return Err("Directory is not accessible (check permissions)".to_string());
    }

    let session_manager = get_session_manager().map_err(|e| e.to_string())?;

    // Ensure session workspace exists in pool (triggers lazy loading)
    let _workspace_path = session_manager.get_session_workspace_dir_by_id(&session_id);

    session_manager
        .set_workspace_override(&session_id, override_path)
        .await
}

#[tauri::command]
pub async fn cancel_workspace_override(session_id: String) -> Result<(), String> {
    let session_manager = get_session_manager().map_err(|e| e.to_string())?;

    // Ensure session workspace exists in pool (triggers lazy loading)
    let _workspace_path = session_manager.get_session_workspace_dir_by_id(&session_id);

    session_manager.remove_workspace_override(&session_id).await
}
