/// Workspace-related Tauri commands
///
/// This module contains commands for workspace and application directory management,
/// including file listing, data directories, and log directories.
use crate::services::{WorkspaceFileItem, WorkspaceService};
use crate::session::get_session_manager;

/// A simple command to test the frontend-backend connection.
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
}

/// Restarts the application process.
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err(
            "In development mode, process restart is not supported. Use reload or restart `tauri dev`."
                .to_string(),
        );
    }

    app.restart();
}

/// Lists files and directories in the current session's workspace.
#[tauri::command]
pub async fn list_workspace_files(
    path: Option<String>,
    session_id: Option<String>,
) -> Result<Vec<WorkspaceFileItem>, String> {
    WorkspaceService::list_files(path, session_id).await
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
#[tauri::command]
pub async fn open_workspace_file_with_default_app(
    file_path: String,
    session_id: Option<String>,
) -> Result<(), String> {
    WorkspaceService::open_file_with_default_app(file_path, session_id).await
}

#[tauri::command]
pub async fn open_workspace_in_explorer(session_id: String) -> Result<(), String> {
    let session_manager = get_session_manager().map_err(|e| e.to_string())?;
    let workspace_path = session_manager.get_session_workspace_dir_by_id(&session_id);

    crate::utils::fs::open_in_file_manager(&workspace_path)
}

#[tauri::command]
pub async fn open_workspace_in_terminal(session_id: String) -> Result<(), String> {
    let session_manager = get_session_manager().map_err(|e| e.to_string())?;
    let workspace_path = session_manager.get_session_workspace_dir_by_id(&session_id);

    crate::utils::terminal::open_in_terminal(&workspace_path)
}

#[tauri::command]
pub async fn get_workspace_override(session_id: String) -> Result<Option<String>, String> {
    WorkspaceService::get_override(&session_id).await
}

#[tauri::command]
pub async fn set_workspace_override(
    session_id: String,
    override_path: String,
) -> Result<(), String> {
    WorkspaceService::set_override(&session_id, override_path).await
}

#[tauri::command]
pub async fn cancel_workspace_override(session_id: String) -> Result<(), String> {
    WorkspaceService::cancel_override(&session_id).await
}
