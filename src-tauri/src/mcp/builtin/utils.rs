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
    base_dir: PathBuf,
}

impl SecurityValidator {
    pub fn new() -> Self {
        let base_dir = if let Ok(root) = std::env::var("SYNAPTICFLOW_PROJECT_ROOT") {
            // 1. 명시적 프로젝트 루트 환경변수
            PathBuf::from(root)
        } else {
            // 2. 확실한 앱 전용 워크스페이스 사용
            let tmp = std::env::temp_dir().join("synaptic-flow");

            // 디렉터리 생성 확인
            if let Err(e) = std::fs::create_dir_all(&tmp) {
                tracing::error!("Failed to create app workspace: {:?}: {}", tmp, e);
            }

            tracing::info!("Using app workspace: {:?}", tmp);
            tmp
        };

        tracing::info!("SecurityValidator base_dir = {:?}", base_dir);

        Self { base_dir }
    }

    /// Validate and clean a file path to prevent directory traversal
    pub fn validate_path(&self, user_path: &str) -> Result<PathBuf, SecurityError> {
        // 디버깅을 위한 로깅 추가
        tracing::debug!(
            "Validating path: '{}' against base: '{:?}'",
            user_path,
            self.base_dir
        );

        // Clean the path to resolve . and .. components
        let clean_path = PathBuf::from(user_path).clean();

        // 절대경로 금지 - 보안 강화
        if clean_path.is_absolute() {
            return Err(SecurityError::PathTraversal(format!(
                "Absolute paths not allowed: '{user_path}'"
            )));
        }

        // Windows 드라이브 경로 금지 (C:, D: 등)
        if user_path.len() >= 2 && user_path.chars().nth(1) == Some(':') {
            return Err(SecurityError::PathTraversal(format!(
                "Windows drive paths not allowed: '{user_path}'"
            )));
        }

        // 상위 디렉터리 탐색 금지
        if user_path.contains("..") {
            return Err(SecurityError::PathTraversal(format!(
                "Parent directory traversal not allowed: '{user_path}'"
            )));
        }

        // base_dir 기준 상대경로로만 처리
        let absolute_path = self.base_dir.join(clean_path);

        tracing::debug!("Resolved path: '{:?}'", absolute_path);

        // 부모 디렉터리 생성 (쓰기 작업을 위해)
        if let Some(parent) = absolute_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Err(SecurityError::InvalidPath(format!(
                    "Failed to create directory: {e}"
                )));
            }
        }

        // 정규화하여 심볼릭 링크 공격 방지
        let canonical_path = match absolute_path.canonicalize() {
            Ok(path) => path,
            Err(_) => {
                // 파일이 존재하지 않는 경우 (쓰기 작업에서 발생 가능)
                tracing::debug!(
                    "File doesn't exist yet, using non-canonical path: '{:?}'",
                    absolute_path
                );
                absolute_path.clone()
            }
        };

        // 최종 검증: base_dir 하위인지 확인
        if !canonical_path.starts_with(&self.base_dir) && !absolute_path.starts_with(&self.base_dir)
        {
            return Err(SecurityError::PathTraversal(format!(
                "Path '{}' resolves outside allowed directory. Base: {:?}, Resolved: {:?}",
                user_path, self.base_dir, canonical_path
            )));
        }

        tracing::debug!("Path validation successful: '{:?}'", absolute_path);
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

    /// Get the base directory
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

impl Default for SecurityValidator {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn test_path_validation() {
        let validator = SecurityValidator::new();

        // Valid paths
        assert!(validator.validate_path("test.txt").is_ok());
        assert!(validator.validate_path("./test.txt").is_ok());
        assert!(validator.validate_path("subdir/test.txt").is_ok());

        // Invalid paths (directory traversal)
        assert!(validator.validate_path("../test.txt").is_err());
        assert!(validator.validate_path("../../etc/passwd").is_err());

        // Invalid paths (absolute paths) - 새로 추가된 보안 검증
        assert!(validator.validate_path("/etc/passwd").is_err());
        assert!(validator.validate_path("/Users/test/file.txt").is_err());
        assert!(validator.validate_path("/tmp/outside.txt").is_err());

        // Invalid paths (Windows drive letters) - 추가된 검증
        assert!(validator.validate_path("C:\\Windows\\System32").is_err());
        assert!(validator.validate_path("D:\\secret.txt").is_err());

        // Invalid paths (complex traversal attempts)
        assert!(validator
            .validate_path("./subdir/../../../etc/passwd")
            .is_err());

        // Windows 스타일 경로도 상대경로로 처리되지만, ".." 포함으로 차단됨
        assert!(validator.validate_path("subdir\\..\\..\\Windows").is_err());
    }
}
