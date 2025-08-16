use path_clean::PathClean;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SecurityError {
    #[error("Path traversal attempt detected: {0}")]
    PathTraversal(String),
    #[error("Access denied: {0}")]
    #[allow(dead_code)]
    AccessDenied(String),
    #[error("File size limit exceeded: {0} bytes")]
    FileSizeLimit(usize),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

/// Security utilities for built-in servers
pub struct SecurityValidator {
    working_dir: PathBuf,
}

impl SecurityValidator {
    pub fn new() -> Result<Self, SecurityError> {
        let working_dir = std::env::current_dir().map_err(|e| {
            SecurityError::InvalidPath(format!("Cannot get current directory: {}", e))
        })?;

        Ok(Self { working_dir })
    }

    /// Validate and clean a file path to prevent directory traversal
    pub fn validate_path(&self, user_path: &str) -> Result<PathBuf, SecurityError> {
        // Clean the path to resolve . and .. components
        let clean_path = PathBuf::from(user_path).clean();

        // Convert to absolute path relative to working directory
        let absolute_path = if clean_path.is_absolute() {
            clean_path
        } else {
            self.working_dir.join(clean_path)
        };

        // Ensure the path is within the working directory
        if !absolute_path.starts_with(&self.working_dir) {
            return Err(SecurityError::PathTraversal(format!(
                "Path '{}' attempts to access outside working directory",
                user_path
            )));
        }

        Ok(absolute_path)
    }

    /// Check if file size is within limits
    pub fn validate_file_size(&self, path: &Path, max_size: usize) -> Result<(), SecurityError> {
        if let Ok(metadata) = std::fs::metadata(path) {
            let file_size = metadata.len() as usize;
            if file_size > max_size {
                return Err(SecurityError::FileSizeLimit(file_size));
            }
        }
        Ok(())
    }

    /// Get the working directory
    #[allow(dead_code)]
    pub fn working_dir(&self) -> &Path {
        &self.working_dir
    }
}

impl Default for SecurityValidator {
    fn default() -> Self {
        Self::new().expect("Failed to create security validator")
    }
}

/// Common constants for built-in servers
pub mod constants {
    /// Maximum file size for reading (10MB)
    pub const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

    /// Maximum code size for sandbox execution (10KB)
    pub const MAX_CODE_SIZE: usize = 10 * 1024;

    /// Default timeout for code execution (30 seconds)
    pub const DEFAULT_EXECUTION_TIMEOUT: u64 = 30;

    /// Maximum execution timeout (60 seconds)
    pub const MAX_EXECUTION_TIMEOUT: u64 = 60;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_path_validation() {
        let validator = SecurityValidator::new().unwrap();

        // Valid paths
        assert!(validator.validate_path("test.txt").is_ok());
        assert!(validator.validate_path("./test.txt").is_ok());
        assert!(validator.validate_path("subdir/test.txt").is_ok());

        // Invalid paths (directory traversal)
        assert!(validator.validate_path("../test.txt").is_err());
        assert!(validator.validate_path("../../etc/passwd").is_err());
        assert!(validator.validate_path("/etc/passwd").is_err());
    }
}
