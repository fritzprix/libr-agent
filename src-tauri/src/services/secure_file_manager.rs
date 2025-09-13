use tokio::fs;
use tracing::{error, info};

use crate::mcp::builtin::utils::{constants::MAX_FILE_SIZE, SecurityValidator};

pub struct SecureFileManager {
    security: SecurityValidator,
}

impl SecureFileManager {
    pub fn new() -> Self {
        Self {
            security: SecurityValidator::new(),
        }
    }

    pub fn new_with_base_dir(base_dir: std::path::PathBuf) -> Self {
        Self {
            security: SecurityValidator::new_with_base_dir(base_dir),
        }
    }

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

    pub async fn write_file_string(&self, path: &str, content: &str) -> Result<(), String> {
        self.write_file(path, content.as_bytes()).await
    }

    pub async fn append_file_string(&self, path: &str, content: &str) -> Result<(), String> {
        let safe_path = self
            .security
            .validate_path(path)
            .map_err(|e| format!("Security error: {e}"))?;

        // 파일이 존재하지 않으면 생성
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

    pub fn get_security_validator(&self) -> &SecurityValidator {
        &self.security
    }
}

impl Default for SecureFileManager {
    fn default() -> Self {
        Self::new()
    }
}
