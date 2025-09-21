use tokio::fs;
use tracing::{error, info};

use crate::mcp::builtin::utils::{constants::MAX_FILE_SIZE, SecurityValidator};

/// Provides secure file system operations by ensuring that all paths are
/// validated and constrained within a specific base directory.
pub struct SecureFileManager {
    security: SecurityValidator,
}

impl SecureFileManager {
    /// Creates a new `SecureFileManager` with the default security settings.
    pub fn new() -> Self {
        Self {
            security: SecurityValidator::new(),
        }
    }

    /// Creates a new `SecureFileManager` with a specified base directory.
    ///
    /// # Arguments
    /// * `base_dir` - The base directory to which all file operations will be restricted.
    pub fn new_with_base_dir(base_dir: std::path::PathBuf) -> Self {
        Self {
            security: SecurityValidator::new_with_base_dir(base_dir),
        }
    }

    /// Securely reads the contents of a file as a byte vector.
    ///
    /// It performs security checks to validate the path and file size before reading.
    ///
    /// # Arguments
    /// * `path` - The relative path to the file.
    ///
    /// # Returns
    /// A `Result` containing the file's byte content, or an error string on failure.
    pub async fn read_file(&self, path: &str) -> Result<Vec<u8>, String> {
        let safe_path = self
            .security
            .validate_path(path)
            .map_err(|e| format!("Security error: {e}"))?;

        // Check if file exists and is a file
        if !safe_path.exists() {
            return Err(format!("File does not exist: {path}"));
        }

        if !safe_path.is_file() {
            return Err(format!("Path is not a file: {path}"));
        }

        // Check file size
        if let Err(e) = self.security.validate_file_size(&safe_path, MAX_FILE_SIZE) {
            return Err(format!("File size error: {e}"));
        }

        // Read the file contents
        fs::read(&safe_path)
            .await
            .map_err(|e| format!("Failed to read file: {e}"))
    }

    /// Securely writes a byte slice to a file.
    ///
    /// It performs security checks to validate the path and content size before writing.
    /// It will also create any necessary parent directories.
    ///
    /// # Arguments
    /// * `path` - The relative path to the file.
    /// * `content` - The byte slice to write to the file.
    ///
    /// # Returns
    /// An empty `Result` on success, or an error string on failure.
    pub async fn write_file(&self, path: &str, content: &[u8]) -> Result<(), String> {
        let safe_path = self
            .security
            .validate_path(path)
            .map_err(|e| format!("Security error: {e}"))?;

        // Check content size
        if content.len() > MAX_FILE_SIZE {
            return Err(format!(
                "Content too large: {} bytes (max: {} bytes)",
                content.len(),
                MAX_FILE_SIZE
            ));
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = safe_path.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                error!("Failed to create parent directory {:?}: {}", parent, e);
                return Err(format!("Failed to create parent directory: {e}"));
            }
        }

        // Write file
        fs::write(&safe_path, content)
            .await
            .map_err(|e| format!("Failed to write file: {e}"))?;

        info!("Successfully wrote file: {:?}", safe_path);
        Ok(())
    }

    /// Securely reads the contents of a file as a `String`.
    ///
    /// # Arguments
    /// * `path` - The relative path to the file.
    ///
    /// # Returns
    /// A `Result` containing the file's content as a string, or an error string on failure.
    pub async fn read_file_as_string(&self, path: &str) -> Result<String, String> {
        let safe_path = self
            .security
            .validate_path(path)
            .map_err(|e| format!("Security error: {e}"))?;

        // Check if file exists and is a file
        if !safe_path.exists() {
            return Err(format!("File does not exist: {path}"));
        }

        if !safe_path.is_file() {
            return Err(format!("Path is not a file: {path}"));
        }

        // Check file size
        if let Err(e) = self.security.validate_file_size(&safe_path, MAX_FILE_SIZE) {
            return Err(format!("File size error: {e}"));
        }

        // Read the file contents as string
        fs::read_to_string(&safe_path)
            .await
            .map_err(|e| format!("Failed to read file: {e}"))
    }

    /// Securely writes a string to a file.
    ///
    /// This is a convenience wrapper around `write_file`.
    ///
    /// # Arguments
    /// * `path` - The relative path to the file.
    /// * `content` - The string content to write.
    ///
    /// # Returns
    /// An empty `Result` on success, or an error string on failure.
    pub async fn write_file_string(&self, path: &str, content: &str) -> Result<(), String> {
        self.write_file(path, content.as_bytes()).await
    }

    /// Securely appends a string to a file.
    ///
    /// If the file does not exist, it will be created.
    ///
    /// # Arguments
    /// * `path` - The relative path to the file.
    /// * `content` - The string content to append.
    ///
    /// # Returns
    /// An empty `Result` on success, or an error string on failure.
    pub async fn append_file_string(&self, path: &str, content: &str) -> Result<(), String> {
        let safe_path = self
            .security
            .validate_path(path)
            .map_err(|e| format!("Security error: {e}"))?;

        // If the file does not exist, create it by writing the content.
        if !safe_path.exists() {
            return self.write_file_string(path, content).await;
        }

        // Open the existing file and append content
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(&safe_path)
            .await
            .map_err(|e| format!("Failed to open file for append: {e}"))?;

        file.write_all(content.as_bytes())
            .await
            .map_err(|e| format!("Failed to append to file: {e}"))?;

        info!("Successfully appended to file: {:?}", safe_path);
        Ok(())
    }

    /// Securely copies a file from an external path to a relative path within the base directory.
    ///
    /// # Arguments
    /// * `src_path` - The absolute path of the source file.
    /// * `dest_rel_path` - The destination path relative to the secure base directory.
    ///
    /// # Returns
    /// A `Result` containing the absolute path of the newly created file, or an error string on failure.
    pub async fn copy_file_from_external(
        &self,
        src_path: &std::path::Path,
        dest_rel_path: &str,
    ) -> Result<std::path::PathBuf, String> {
        // Validate destination path using security validator
        let dest_path = self
            .security
            .validate_path(dest_rel_path)
            .map_err(|e| format!("Security error for destination: {e}"))?;

        // Check source file exists and is a file
        if !src_path.exists() {
            return Err(format!(
                "Source file does not exist: {}",
                src_path.display()
            ));
        }

        if !src_path.is_file() {
            return Err(format!("Source path is not a file: {}", src_path.display()));
        }

        // Check source file size
        if let Err(e) = self.security.validate_file_size(src_path, MAX_FILE_SIZE) {
            return Err(format!("Source file size error: {e}"));
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = dest_path.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                error!("Failed to create parent directory {:?}: {}", parent, e);
                return Err(format!("Failed to create parent directory: {e}"));
            }
        }

        // Copy file
        fs::copy(src_path, &dest_path)
            .await
            .map_err(|e| format!("Failed to copy file: {e}"))?;

        info!(
            "Successfully copied file from {} to {}",
            src_path.display(),
            dest_path.display()
        );

        Ok(dest_path)
    }

    /// Returns a reference to the internal `SecurityValidator`.
    pub fn get_security_validator(&self) -> &SecurityValidator {
        &self.security
    }
}

impl Default for SecureFileManager {
    fn default() -> Self {
        Self::new()
    }
}
