/// File operation commands
///
/// This module contains commands for reading and writing files in the workspace,
/// including secure file operations and dropped file handling.
use crate::services::SecureFileManager;
use crate::session::get_session_manager;
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use tokio::fs;
use tokio::io::AsyncReadExt;

static DROPPED_FILE_ALLOWLIST: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn dropped_file_allowlist() -> &'static Mutex<HashSet<String>> {
    DROPPED_FILE_ALLOWLIST.get_or_init(|| Mutex::new(HashSet::new()))
}

fn has_hidden_or_relative_component(path: &Path) -> bool {
    path.components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
}

fn has_hidden_component(path: &Path) -> bool {
    path.components().any(|c| {
        let component = c.as_os_str().to_string_lossy();
        !component.is_empty() && component.starts_with('.')
    })
}

/// Registers paths delivered by an OS-level file-drop event.
///
/// These paths are consumed once by `read_dropped_file` to prevent arbitrary path reads
/// from untrusted IPC callers.
#[tauri::command]
pub async fn register_dropped_files(paths: Vec<String>) -> Result<(), String> {
    const MAX_DROPPED_FILE_ALLOWLIST_SIZE: usize = 256;

    let mut normalized_paths = Vec::new();
    for path_str in paths {
        let path = Path::new(&path_str);
        if !path.exists() || !path.is_file() {
            continue;
        }

        let Ok(metadata) = std::fs::symlink_metadata(path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }

        if has_hidden_or_relative_component(path) {
            continue;
        }

        let Ok(resolved_path) = std::fs::canonicalize(path) else {
            continue;
        };

        if has_hidden_component(&resolved_path) {
            continue;
        }

        normalized_paths.push(resolved_path.to_string_lossy().to_string());
    }

    let allowlist = dropped_file_allowlist();
    let mut guard = allowlist
        .lock()
        .map_err(|_| "Dropped file allowlist lock poisoned".to_string())?;

    if guard.len() >= MAX_DROPPED_FILE_ALLOWLIST_SIZE {
        guard.clear();
    }

    for path in normalized_paths {
        if guard.len() >= MAX_DROPPED_FILE_ALLOWLIST_SIZE {
            break;
        }
        guard.insert(path);
    }

    Ok(())
}

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

    // Security check: reject hidden files/directories and traversal/current-dir style components
    if has_hidden_or_relative_component(path) {
        return Err("Access denied: Hidden files and directories are not allowed".to_string());
    }

    // Security check: reject direct symlink paths (fs::read would follow links)
    let symlink_metadata = std::fs::symlink_metadata(path)
        .map_err(|e| format!("Failed to inspect file metadata: {e}"))?;
    if symlink_metadata.file_type().is_symlink() {
        return Err("Access denied: Symbolic links are not allowed".to_string());
    }

    // Resolve final path and enforce one-time OS-drop allowlist.
    let resolved_path = std::fs::canonicalize(path)
        .map_err(|e| format!("Failed to resolve dropped file path: {e}"))?;

    if has_hidden_component(&resolved_path) {
        return Err("Access denied: Hidden files and directories are not allowed".to_string());
    }

    let resolved_path_str = resolved_path.to_string_lossy().to_string();
    {
        let allowlist = dropped_file_allowlist();
        let mut guard = allowlist
            .lock()
            .map_err(|_| "Dropped file allowlist lock poisoned".to_string())?;

        if !guard.remove(&resolved_path_str) {
            return Err(
                "Access denied: File path was not provided by an OS file-drop event".to_string(),
            );
        }
    }

    // Check file size
    if let Ok(metadata) = fs::metadata(&resolved_path).await {
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
    let extension = resolved_path
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
    let file = fs::File::open(&resolved_path)
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
    use tempfile::{tempdir, Builder};

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    fn reset_allowlist_for_test() {
        let allowlist = dropped_file_allowlist();
        let mut guard = allowlist
            .lock()
            .expect("Dropped file allowlist lock should not be poisoned in tests");
        guard.clear();
    }

    async fn register_for_test(path: &Path) {
        register_dropped_files(vec![path.to_string_lossy().to_string()])
            .await
            .expect("register_dropped_files should succeed in tests");
    }

    #[tokio::test]
    async fn test_read_dropped_file_rejects_hidden() {
        reset_allowlist_for_test();
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
        register_for_test(&hidden_file).await;
        let result = read_dropped_file(hidden_file.to_string_lossy().to_string()).await;
        assert!(result.is_err(), "Hidden file should be rejected");
        assert!(
            result.unwrap_err().contains("Access denied"),
            "Error should mention access denied"
        );

        // Test file in hidden directory
        register_for_test(&file_in_hidden).await;
        let result = read_dropped_file(file_in_hidden.to_string_lossy().to_string()).await;
        assert!(
            result.is_err(),
            "File in hidden directory should be rejected"
        );
        assert!(
            result.unwrap_err().contains("Access denied"),
            "Error should mention access denied"
        );

        // Test normal file
        register_for_test(&normal_file).await;
        let result = read_dropped_file(normal_file.to_string_lossy().to_string()).await;
        assert!(result.is_ok(), "Normal file should be accepted");
    }

    #[tokio::test]
    async fn test_read_dropped_file_rejects_relative_components() {
        reset_allowlist_for_test();
        let dir = tempdir().unwrap();
        let normal_file = dir.path().join("normal.txt");
        std::fs::write(&normal_file, "ok").unwrap();

        let dotted_path = dir.path().join(".").join("normal.txt");
        register_for_test(&dotted_path).await;
        let result = read_dropped_file(dotted_path.to_string_lossy().to_string()).await;
        assert!(
            result.is_err(),
            "Path containing '.' component should be rejected"
        );
        assert!(result.unwrap_err().contains("Access denied"));

        let child_dir = dir.path().join("child");
        std::fs::create_dir(&child_dir).unwrap();
        let parent_path = child_dir.join("..").join("normal.txt");
        register_for_test(&parent_path).await;
        let result = read_dropped_file(parent_path.to_string_lossy().to_string()).await;
        assert!(
            result.is_err(),
            "Path containing '..' component should be rejected"
        );
        assert!(result.unwrap_err().contains("Access denied"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_read_dropped_file_rejects_symlink_targets() {
        reset_allowlist_for_test();
        let dir = tempdir().unwrap();
        let hidden_dir = dir.path().join(".hidden");
        std::fs::create_dir(&hidden_dir).unwrap();

        let hidden_target = hidden_dir.join("secret.txt");
        std::fs::write(&hidden_target, "secret").unwrap();

        let symlink_path = dir.path().join("visible.txt");
        symlink(&hidden_target, &symlink_path).unwrap();

        register_for_test(&symlink_path).await;
        let result = read_dropped_file(symlink_path.to_string_lossy().to_string()).await;
        assert!(result.is_err(), "Symlink should be rejected");
        assert!(result.unwrap_err().contains("Symbolic links"));
    }

    #[tokio::test]
    async fn test_read_dropped_file_requires_registered_drop_path() {
        reset_allowlist_for_test();
        let dir = tempdir().unwrap();
        let normal_file = dir.path().join("normal.txt");
        std::fs::write(&normal_file, "ok").unwrap();

        let result = read_dropped_file(normal_file.to_string_lossy().to_string()).await;
        assert!(result.is_err(), "Unregistered path should be rejected");
        assert!(result.unwrap_err().contains("OS file-drop"));

        register_for_test(&normal_file).await;
        let first = read_dropped_file(normal_file.to_string_lossy().to_string()).await;
        assert!(first.is_ok(), "Registered path should be accepted once");

        let second = read_dropped_file(normal_file.to_string_lossy().to_string()).await;
        assert!(
            second.is_err(),
            "Path should be consumed and rejected on second read"
        );
    }

    #[tokio::test]
    async fn test_register_dropped_files_caps_allowlist_size() {
        reset_allowlist_for_test();
        let current_dir = std::env::current_dir().unwrap();
        let dir = Builder::new()
            .prefix("allowlist-capacity-test-")
            .tempdir_in(current_dir)
            .unwrap();

        let mut paths = Vec::new();
        for index in 0..300 {
            let file_path = dir.path().join(format!("file-{index}.txt"));
            std::fs::write(&file_path, "ok").unwrap();
            paths.push(file_path.to_string_lossy().to_string());
        }

        register_dropped_files(paths)
            .await
            .expect("register_dropped_files should succeed");

        let allowlist = dropped_file_allowlist();
        let guard = allowlist
            .lock()
            .expect("Dropped file allowlist lock should not be poisoned in tests");

        assert!(
            guard.len() <= 256,
            "allowlist size must never exceed max capacity"
        );
        assert_eq!(guard.len(), 256, "allowlist should stop at max capacity");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_register_dropped_files_rejects_symlink_paths() {
        reset_allowlist_for_test();
        let dir = tempdir().unwrap();

        let target = dir.path().join("target.txt");
        std::fs::write(&target, "ok").unwrap();

        let symlink_path = dir.path().join("link.txt");
        symlink(&target, &symlink_path).unwrap();

        register_dropped_files(vec![symlink_path.to_string_lossy().to_string()])
            .await
            .expect("register_dropped_files should not fail for symlink input");

        let resolved_target = std::fs::canonicalize(&target)
            .unwrap()
            .to_string_lossy()
            .to_string();

        let allowlist = dropped_file_allowlist();
        let guard = allowlist
            .lock()
            .expect("Dropped file allowlist lock should not be poisoned in tests");

        assert!(
            !guard.contains(&resolved_target),
            "symlink registration must not add canonical target to allowlist"
        );
    }
}
