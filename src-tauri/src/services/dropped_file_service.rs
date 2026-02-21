use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;
use tokio::fs;
use tokio::io::AsyncReadExt;

/// Service for handling files dropped onto the application window.
///
/// This service enforces security policies to prevent arbitrary file access:
/// - Maintains an allowlist of paths provided by the OS file-drop event.
/// - Validates file types, extensions, and sizes.
/// - Prevents access to hidden files and symlinks.
pub struct DroppedFileService {
    allowlist: Mutex<HashSet<String>>,
}

impl DroppedFileService {
    pub fn new() -> Self {
        Self {
            allowlist: Mutex::new(HashSet::new()),
        }
    }

    /// Registers paths delivered by an OS-level file-drop event.
    ///
    /// These paths are consumed once by `read_dropped_file`.
    pub async fn register_dropped_files(&self, paths: Vec<String>) -> Result<(), String> {
        const MAX_DROPPED_FILE_ALLOWLIST_SIZE: usize = 256;

        let mut normalized_paths = Vec::new();
        for path_str in paths {
            let path = Path::new(&path_str);
            if !path.exists() || !path.is_file() {
                continue;
            }

            // Symlink check
            if let Ok(metadata) = std::fs::symlink_metadata(path) {
                if metadata.file_type().is_symlink() {
                    continue;
                }
            } else {
                continue;
            }

            if self.has_hidden_or_relative_component(path) {
                continue;
            }

            if let Ok(resolved_path) = std::fs::canonicalize(path) {
                if self.has_hidden_component(&resolved_path) {
                    continue;
                }
                normalized_paths.push(resolved_path.to_string_lossy().to_string());
            }
        }

        let mut guard = self
            .allowlist
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
    /// Performs security checks and enforces the allowlist.
    pub async fn read_dropped_file(&self, file_path: String) -> Result<Vec<u8>, String> {
        let path = Path::new(&file_path);

        // Basic security checks
        if !path.exists() {
            return Err(format!("File does not exist: {file_path}"));
        }

        if !path.is_file() {
            return Err(format!("Path is not a file: {file_path}"));
        }

        if self.has_hidden_or_relative_component(path) {
            return Err("Access denied: Hidden files and directories are not allowed".to_string());
        }

        // Symlink check
        let symlink_metadata = std::fs::symlink_metadata(path)
            .map_err(|e| format!("Failed to inspect file metadata: {e}"))?;
        if symlink_metadata.file_type().is_symlink() {
            return Err("Access denied: Symbolic links are not allowed".to_string());
        }

        // Resolve and check allowlist
        let resolved_path = std::fs::canonicalize(path)
            .map_err(|e| format!("Failed to resolve dropped file path: {e}"))?;

        if self.has_hidden_component(&resolved_path) {
            return Err("Access denied: Hidden files and directories are not allowed".to_string());
        }

        let resolved_path_str = resolved_path.to_string_lossy().to_string();
        {
            let mut guard = self
                .allowlist
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
            let max_size = crate::config::max_file_size() as u64;
            if metadata.len() > max_size {
                return Err(format!(
                    "File too large: {} bytes (max: {} bytes)",
                    metadata.len(),
                    max_size
                ));
            }
        }

        // Check extension
        let allowed_extensions = ["txt", "md", "json", "pdf", "docx", "xlsx"];
        let extension = resolved_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase());

        match extension {
            Some(ext) if allowed_extensions.contains(&ext.as_str()) => {
                // Allowed
            }
            _ => {
                return Err(format!(
                    "File type not allowed. Supported: {}",
                    allowed_extensions.join(", ")
                ));
            }
        }

        // Read file
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

    fn has_hidden_or_relative_component(&self, path: &Path) -> bool {
        path.components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    }

    fn has_hidden_component(&self, path: &Path) -> bool {
        path.components().any(|c| {
            let component = c.as_os_str().to_string_lossy();
            !component.is_empty() && component.starts_with('.')
        })
    }
}

impl Default for DroppedFileService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::Builder;

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    async fn register_for_test(service: &DroppedFileService, path: &Path) {
        service
            .register_dropped_files(vec![path.to_string_lossy().to_string()])
            .await
            .expect("register_dropped_files should succeed in tests");
    }

    fn setup_test_dir() -> (tempfile::TempDir, std::path::PathBuf) {
        let current_dir = std::env::current_dir().unwrap();
        let dir = Builder::new()
            .prefix("test-dropped-files-")
            .tempdir_in(current_dir)
            .unwrap();
        let test_root = dir.path().to_path_buf();
        (dir, test_root)
    }

    #[tokio::test]
    async fn test_read_dropped_file_rejects_hidden() {
        let service = DroppedFileService::new();
        let (_dir, test_root) = setup_test_dir();

        let hidden_file = test_root.join(".secret.txt");
        File::create(&hidden_file).unwrap();

        let hidden_dir = test_root.join(".hidden");
        std::fs::create_dir(&hidden_dir).unwrap();
        let file_in_hidden = hidden_dir.join("normal.txt");
        File::create(&file_in_hidden).unwrap();

        let normal_file = test_root.join("normal.txt");
        File::create(&normal_file).unwrap();

        register_for_test(&service, &hidden_file).await;
        let result = service
            .read_dropped_file(hidden_file.to_string_lossy().to_string())
            .await;
        assert!(result.is_err(), "Hidden file should be rejected");
        assert!(
            result.unwrap_err().contains("Access denied"),
            "Error should mention access denied"
        );

        register_for_test(&service, &file_in_hidden).await;
        let result = service
            .read_dropped_file(file_in_hidden.to_string_lossy().to_string())
            .await;
        assert!(
            result.is_err(),
            "File in hidden directory should be rejected"
        );
        assert!(
            result.unwrap_err().contains("Access denied"),
            "Error should mention access denied"
        );

        register_for_test(&service, &normal_file).await;
        let result = service
            .read_dropped_file(normal_file.to_string_lossy().to_string())
            .await;
        assert!(result.is_ok(), "Normal file should be accepted: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_read_dropped_file_rejects_relative_components() {
        let service = DroppedFileService::new();
        let (_dir, test_root) = setup_test_dir();
        let normal_file = test_root.join("normal.txt");
        std::fs::write(&normal_file, "ok").unwrap();

        // Note: We don't test "." inside path because Rust's Components iterator
        // suppresses redundant "." components in the middle of paths, making them safe/allowed.

        let child_dir = test_root.join("child");
        std::fs::create_dir(&child_dir).unwrap();
        let parent_path = child_dir.join("..").join("normal.txt");
        register_for_test(&service, &parent_path).await;
        let result = service
            .read_dropped_file(parent_path.to_string_lossy().to_string())
            .await;
        assert!(
            result.is_err(),
            "Path containing '..' component should be rejected"
        );
        assert!(result.unwrap_err().contains("Access denied"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_read_dropped_file_rejects_symlink_targets() {
        let service = DroppedFileService::new();
        let (_dir, test_root) = setup_test_dir();
        let hidden_dir = test_root.join(".hidden");
        std::fs::create_dir(&hidden_dir).unwrap();

        let hidden_target = hidden_dir.join("secret.txt");
        std::fs::write(&hidden_target, "secret").unwrap();

        let symlink_path = test_root.join("visible.txt");
        symlink(&hidden_target, &symlink_path).unwrap();

        register_for_test(&service, &symlink_path).await;
        let result = service
            .read_dropped_file(symlink_path.to_string_lossy().to_string())
            .await;
        assert!(result.is_err(), "Symlink should be rejected");
        let err = result.unwrap_err();
        assert!(
            err.contains("Symbolic links"),
            "Expected 'Symbolic links' error, got: '{}'",
            err
        );
    }

    #[tokio::test]
    async fn test_read_dropped_file_requires_registered_drop_path() {
        let service = DroppedFileService::new();
        let (_dir, test_root) = setup_test_dir();
        let normal_file = test_root.join("normal.txt");
        std::fs::write(&normal_file, "ok").unwrap();

        let result = service
            .read_dropped_file(normal_file.to_string_lossy().to_string())
            .await;
        assert!(result.is_err(), "Unregistered path should be rejected");
        let err = result.unwrap_err();
        assert!(
            err.contains("OS file-drop"),
            "Expected 'OS file-drop' error, got: '{}'",
            err
        );

        register_for_test(&service, &normal_file).await;
        let first = service
            .read_dropped_file(normal_file.to_string_lossy().to_string())
            .await;
        assert!(first.is_ok(), "Registered path should be accepted once");

        let second = service
            .read_dropped_file(normal_file.to_string_lossy().to_string())
            .await;
        assert!(
            second.is_err(),
            "Path should be consumed and rejected on second read"
        );
    }

    #[tokio::test]
    async fn test_register_dropped_files_caps_allowlist_size() {
        let service = DroppedFileService::new();
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

        service
            .register_dropped_files(paths)
            .await
            .expect("register_dropped_files should succeed");

        let guard = service.allowlist.lock().unwrap();
        assert!(
            guard.len() <= 256,
            "allowlist size must never exceed max capacity"
        );
        assert_eq!(guard.len(), 256, "allowlist should stop at max capacity");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_register_dropped_files_rejects_symlink_paths() {
        let service = DroppedFileService::new();
        let (_dir, test_root) = setup_test_dir();

        let target = test_root.join("target.txt");
        std::fs::write(&target, "ok").unwrap();

        let symlink_path = test_root.join("link.txt");
        symlink(&target, &symlink_path).unwrap();

        service
            .register_dropped_files(vec![symlink_path.to_string_lossy().to_string()])
            .await
            .expect("register_dropped_files should not fail for symlink input");

        let resolved_target = std::fs::canonicalize(&target)
            .unwrap()
            .to_string_lossy()
            .to_string();

        let guard = service.allowlist.lock().unwrap();
        assert!(
            !guard.contains(&resolved_target),
            "symlink registration must not add canonical target to allowlist"
        );
    }
}
