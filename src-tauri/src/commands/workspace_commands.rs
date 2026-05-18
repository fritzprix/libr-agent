use crate::services::{WorkspaceFileItem, WorkspaceService};
use crate::session::get_session_manager;
/// Workspace-related Tauri commands
///
/// This module contains commands for workspace and application directory management,
/// including file listing, data directories, and log directories.
use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInstallCapability {
    pub supported: bool,
    pub current_executable: String,
    pub package_type: String,
    pub reason: Option<String>,
}

/// Returns whether this installation can perform an in-app update install.
///
/// On Linux, Tauri's updater is only practical for writable AppImage installs.
/// System package installs (`.deb`/`.rpm`) typically live under root-owned paths
/// and cannot be replaced by an unprivileged desktop process.
#[tauri::command]
pub async fn get_update_install_capability() -> Result<UpdateInstallCapability, String> {
    let current_executable = std::env::current_exe()
        .map_err(|err| format!("Failed to resolve current executable: {err}"))?;
    let current_executable = current_executable.to_string_lossy().to_string();

    #[cfg(target_os = "linux")]
    {
        if let Ok(appimage_path) = std::env::var("APPIMAGE") {
            if !appimage_path.trim().is_empty() {
                return Ok(UpdateInstallCapability {
                    supported: true,
                    current_executable,
                    package_type: "appimage".to_string(),
                    reason: None,
                });
            }
        }

        Ok(UpdateInstallCapability {
            supported: false,
            current_executable: current_executable.clone(),
            package_type: "system-package".to_string(),
            reason: Some(format!(
                "This Linux installation appears to be a system package at `{}`. In-app updates are only supported for AppImage installs; please update with your package manager or download the latest release manually.",
                current_executable
            )),
        })
    }

    #[cfg(not(target_os = "linux"))]
    {
        Ok(UpdateInstallCapability {
            supported: true,
            current_executable,
            package_type: std::env::consts::OS.to_string(),
            reason: None,
        })
    }
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
    let workspace_path =
        crate::session::resolve_session_workspace_dir(session_manager, &session_id).await?;

    crate::utils::fs::open_in_file_manager(&workspace_path)
}

#[tauri::command]
pub async fn open_workspace_in_terminal(session_id: String) -> Result<(), String> {
    let session_manager = get_session_manager().map_err(|e| e.to_string())?;
    let workspace_path =
        crate::session::resolve_session_workspace_dir(session_manager, &session_id).await?;

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

/// Returns the absolute workspace directory path for the given session.
/// Used by the frontend to construct file:// URLs for binary file indexing.
#[tauri::command]
pub async fn get_workspace_dir(session_id: String) -> Result<String, String> {
    let session_manager = get_session_manager().map_err(|e| e.to_string())?;
    let workspace_path =
        crate::session::resolve_session_workspace_dir(session_manager, &session_id).await?;
    Ok(workspace_path.to_string_lossy().to_string())
}

/// Reads a local file URI and returns its contents as a base64 string.
///
/// This exists for frontend multimodal request preparation because webview
/// `fetch(file://...)` is not reliable across platforms/runtime policies.
#[tauri::command]
pub async fn read_local_file_as_base64(
    session_id: String,
    file_url: String,
) -> Result<String, String> {
    let url = url::Url::parse(&file_url).map_err(|e| format!("Invalid file URL format: {e}"))?;

    if url.scheme() != "file" {
        return Err(format!(
            "read_local_file_as_base64 only supports file:// URLs, got: {}",
            url.scheme()
        ));
    }

    let file_path = url
        .to_file_path()
        .map_err(|_| "URL cannot be converted to a local file path".to_string())?;

    let session_manager = get_session_manager().map_err(|e| e.to_string())?;
    let workspace_dir =
        crate::session::resolve_session_workspace_dir(session_manager, &session_id).await?;
    let file_path = resolve_workspace_scoped_file_path(&file_path, &workspace_dir).await?;

    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|e| format!("Failed to read local file '{}': {e}", file_path.display()))?;

    Ok(general_purpose::STANDARD.encode(bytes))
}

#[tauri::command]
pub async fn list_workspace_file_paths(
    session_id: String,
    max_depth: usize,
) -> Result<Vec<String>, String> {
    crate::agent::references::list_workspace_relative_paths(&session_id, max_depth).await
}

#[tauri::command]
pub async fn list_workspace_file_paths_for_path(
    workspace_path: String,
    max_depth: usize,
) -> Result<Vec<String>, String> {
    crate::agent::references::list_relative_paths_in_root(Path::new(&workspace_path), max_depth)
        .await
}

pub async fn resolve_workspace_scoped_file_path(
    file_path: &Path,
    workspace_dir: &Path,
) -> Result<PathBuf, String> {
    let canonical_workspace = tokio::fs::canonicalize(workspace_dir).await.map_err(|e| {
        format!(
            "Failed to resolve workspace directory '{}': {e}",
            workspace_dir.display()
        )
    })?;
    let canonical_file = tokio::fs::canonicalize(file_path).await.map_err(|e| {
        format!(
            "Failed to resolve local file '{}': {e}",
            file_path.display()
        )
    })?;

    if !canonical_file.starts_with(&canonical_workspace) {
        return Err(format!(
            "Local file '{}' is outside the session workspace '{}'",
            canonical_file.display(),
            canonical_workspace.display()
        ));
    }

    Ok(canonical_file)
}
