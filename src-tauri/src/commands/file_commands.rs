/// File operation commands
///
/// This module contains commands for reading and writing files in the workspace,
/// including secure file operations and dropped file handling.
use crate::services::{DroppedFileService, SecureFileManager};
use crate::session::get_session_manager;
use tauri::State;

/// Registers paths delivered by an OS-level file-drop event.
///
/// These paths are consumed once by `read_dropped_file` to prevent arbitrary path reads
/// from untrusted IPC callers.
#[tauri::command]
pub async fn register_dropped_files(
    service: State<'_, DroppedFileService>,
    paths: Vec<String>,
) -> Result<(), String> {
    service.register_dropped_files(paths).await
}

/// Reads a file that was dropped onto the application window.
///
/// This function delegates to `DroppedFileService` which performs security checks:
/// - Verifies the file exists and is a file.
/// - Enforces a maximum file size.
/// - Restricts allowed file extensions to a predefined list.
/// - Enforces the allowlist populated by `register_dropped_files`.
///
/// # Arguments
/// * `file_path` - The absolute path of the dropped file.
///
/// # Returns
/// A `Result` containing the file's raw byte content, or an error string if a check fails.
#[tauri::command]
pub async fn read_dropped_file(
    service: State<'_, DroppedFileService>,
    file_path: String,
) -> Result<Vec<u8>, String> {
    service.read_dropped_file(file_path).await
}

/// Writes content to a file in the workspace using the `SecureFileManager`.
#[tauri::command]
pub async fn write_file(
    file_path: String,
    content: Vec<u8>,
    manager: State<'_, SecureFileManager>,
) -> Result<(), String> {
    manager.write_file(&file_path, &content).await
}

/// A session-aware command to write a file to the current session's workspace.
///
/// This ensures that file operations are contained within the active session's
/// designated workspace directory, preventing writes to unintended locations.
#[tauri::command]
pub async fn workspace_write_file(
    file_path: String,
    content: Vec<u8>,
    session_id: Option<String>,
) -> Result<(), String> {
    let session_manager =
        get_session_manager().map_err(|e| format!("Session manager error: {e}"))?;

    // Session ID is mandatory for workspace operations in V2 logic
    if let Some(sid) = session_id {
        let workspace_dir = session_manager.get_session_workspace_dir_by_id(&sid);
        // Create a temporary secure file manager for this operation
        let manager = crate::services::SecureFileManager::new_with_base_dir(workspace_dir);
        return manager.write_file(&file_path, &content).await;
    }

    Err("Session ID is required for workspace write operations".to_string())
}
