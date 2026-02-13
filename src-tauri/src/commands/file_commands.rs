/// File operation commands
///
/// This module contains commands for reading and writing files in the workspace,
/// including secure file operations and dropped file handling.
use crate::services::SecureFileManager;
use crate::session::get_session_manager;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncReadExt;

/// Reads a file that was dropped onto the application window.
///
/// This function performs several security checks:
/// - Verifies the file exists and is a file.
/// - Enforces a maximum file size (100MB, configurable via LIBRAGENT_MAX_FILE_SIZE).
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

    // Security check: reject hidden files/directories (starting with .)
    // This mitigates access to sensitive hidden configurations like ~/.ssh, ~/.aws, ~/.config
    // Also implicitly blocks traversal (..) and current dir (.) components
    if path.components().any(|c| {
        c.as_os_str().to_string_lossy().starts_with('.')
    }) {
        return Err("Access denied: Hidden files and directories are not allowed".to_string());
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

    // Read the file with a size limit to prevent TOCTOU/DoS
    let file = fs::File::open(path)
        .await
        .map_err(|e| format!("Failed to open file: {e}"))?;

    let max_size = crate::config::max_file_size() as u64;
    let read_limit = max_size.saturating_add(1);
    let mut content = Vec::new();

    let bytes_read = file
        .take(read_limit)
        .read_to_end(&mut content)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;

    if bytes_read as u64 > max_size {
        return Err(format!(
            "File exceeds the maximum allowed size of {} bytes",
            max_size
        ));
    }

    Ok(content)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_read_dropped_file_rejects_hidden() {
        let dir = tempdir().unwrap();

        // Create a hidden file
        let hidden_file = dir.path().join(".secret.txt");
        File::create(&hidden_file).unwrap();

        // Create a file in hidden directory
        let hidden_dir = dir.path().join(".hidden");
        std::fs::create_dir(&hidden_dir).unwrap();
        let file_in_hidden = hidden_dir.join("normal.txt");
        File::create(&file_in_hidden).unwrap();

        // Create a normal file
        let normal_file = dir.path().join("normal.txt");
        File::create(&normal_file).unwrap();

        // Test hidden file
        let result = read_dropped_file(hidden_file.to_string_lossy().to_string()).await;
        assert!(result.is_err(), "Hidden file should be rejected");
        assert!(result.unwrap_err().contains("Access denied"), "Error should mention access denied");

        // Test file in hidden directory
        let result = read_dropped_file(file_in_hidden.to_string_lossy().to_string()).await;
        assert!(result.is_err(), "File in hidden directory should be rejected");
        assert!(result.unwrap_err().contains("Access denied"), "Error should mention access denied");

        // Test normal file
        let result = read_dropped_file(normal_file.to_string_lossy().to_string()).await;
        assert!(result.is_ok(), "Normal file should be accepted");
    }
}
