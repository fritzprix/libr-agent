/// File operation commands
///
/// This module contains commands for reading and writing files in the workspace,
/// including secure file operations and dropped file handling.
use crate::services::SecureFileManager;
use crate::session::get_session_manager;
use std::path::Path;
use tokio::fs;

/// Reads a file from the workspace using the `SecureFileManager`.
#[tauri::command]
pub async fn read_file(
    file_path: String,
    manager: tauri::State<'_, SecureFileManager>,
) -> Result<Vec<u8>, String> {
    manager.read_file(&file_path).await
}

/// Reads a file that was dropped onto the application window.
///
/// This function performs several security checks:
/// - Verifies the file exists and is a file.
/// - Enforces a maximum file size (10MB).
/// - Restricts allowed file extensions to a predefined list.
///
/// # Arguments
/// * `file_path` - The absolute path of the dropped file.
///
/// # Returns
/// A `Result` containing the file's raw byte content, or an error string if a check fails.
#[tauri::command]
pub async fn read_dropped_file(file_path: String) -> Result<Vec<u8>, String> {
    let path = Path::new(&file_path);

    // Basic security checks for dropped files
    if !path.exists() {
        return Err(format!("File does not exist: {file_path}"));
    }

    if !path.is_file() {
        return Err(format!("Path is not a file: {file_path}"));
    }

    // Check file size
    if let Ok(metadata) = fs::metadata(path).await {
        // Use runtime-configured max file size (bytes)
        let max_size = crate::config::max_file_size() as u64;
        if metadata.len() > max_size {
            return Err(format!(
                "File too large: {} bytes (max: {} bytes)",
                metadata.len(),
                max_size
            ));
        }
    }

    // Only allow specific file extensions
    let allowed_extensions = ["txt", "md", "json", "pdf", "docx", "xlsx"];
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());

    match extension {
        Some(ext) if allowed_extensions.contains(&ext.as_str()) => {
            // Extension is allowed, proceed with reading
        }
        _ => {
            return Err(format!(
                "File type not allowed. Supported: {}",
                allowed_extensions.join(", ")
            ));
        }
    }

    // Read the file
    fs::read(path)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))
}

/// Writes content to a file in the workspace using the `SecureFileManager`.
#[tauri::command]
pub async fn write_file(
    file_path: String,
    content: Vec<u8>,
    manager: tauri::State<'_, SecureFileManager>,
) -> Result<(), String> {
    manager.write_file(&file_path, &content).await
}

/// A session-aware command to write a file to the current session's workspace.
///
/// This ensures that file operations are contained within the active session's
/// designated workspace directory, preventing writes to unintended locations.
#[tauri::command]
pub async fn workspace_write_file(file_path: String, content: Vec<u8>) -> Result<(), String> {
    let session_manager =
        get_session_manager().map_err(|e| format!("Session manager error: {e}"))?;

    let session_file_manager = session_manager.get_file_manager();
    session_file_manager.write_file(&file_path, &content).await
}
