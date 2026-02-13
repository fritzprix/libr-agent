use path_clean::PathClean;
use std::path::{Component, Path, PathBuf};
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
    pub fn new_with_base_dir(base_dir: PathBuf) -> Self {
        tracing::info!(
            "SecurityValidator created with custom base_dir = {:?}",
            base_dir
        );

        // Ensure the base directory exists
        if let Err(e) = std::fs::create_dir_all(&base_dir) {
            tracing::error!("Failed to create base directory {:?}: {}", base_dir, e);
        }

        // Canonicalize base_dir to resolve any symlinks (e.g. /tmp -> /private/tmp on macOS)
        // This is crucial because validate_path compares canonicalized user paths against this.
        let final_base_dir = match base_dir.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    "Failed to canonicalize base_dir {:?}: {}. Using as-is.",
                    base_dir,
                    e
                );
                base_dir
            }
        };

        Self {
            base_dir: final_base_dir,
        }
    }

    /// Validate and clean a file path to prevent directory traversal
    pub fn validate_path(&self, user_path: &str) -> Result<PathBuf, SecurityError> {
        // 디버깅을 위한 로깅 추가
        tracing::debug!(
            "Validating path: '{}' against base: '{:?}'",
            user_path,
            self.base_dir
        );

        // 경로 구분자 정규화 및 정리
        let normalized_path = user_path.replace('\\', "/");
        let mut clean_path = PathBuf::from(normalized_path).clean();

        // 절대경로 처리: base_dir 내부에 있으면 허용하고 상대경로로 변환
        if clean_path.is_absolute() {
            if clean_path.starts_with(&self.base_dir) {
                match clean_path.strip_prefix(&self.base_dir) {
                    Ok(p) => {
                        clean_path = p.to_path_buf();
                        tracing::debug!("Converted absolute path to relative: {:?}", clean_path);
                    }
                    Err(e) => {
                        return Err(SecurityError::PathTraversal(format!(
                            "Failed to strip prefix from absolute path: {}",
                            e
                        )));
                    }
                }
            } else {
                return Err(SecurityError::PathTraversal(format!(
                    "Absolute paths not allowed (outside workspace): '{user_path}'"
                )));
            }
        } else {
            // Windows 드라이브 경로 금지 (C:, D: 등) - 상대경로인 경우에만 체크
            if user_path.len() >= 2 && user_path.chars().nth(1) == Some(':') {
                return Err(SecurityError::PathTraversal(format!(
                    "Absolute paths with drive letters are not allowed for destination paths: '{user_path}'. \
                     Please use relative paths like 'folder/file.txt'. \
                     The file will be placed inside the workspace directory."
                )));
            }
        }

        // 상위 디렉터리 탐색 금지
        let traversal_check_path = user_path.replace('\\', "/");

        if Path::new(&traversal_check_path)
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(SecurityError::PathTraversal(format!(
                "Parent directory traversal not allowed: '{user_path}'"
            )));
        }

        // base_dir 기준 상대경로로만 처리
        let absolute_path = self.base_dir.join(clean_path);

        tracing::debug!("Resolved path: '{:?}'", absolute_path);

        // 부모 디렉터리 생성 로직 제거 (SecurityValidator는 검증만 수행해야 함)
        // 쓰기 작업 시에는 SecureFileManager가 명시적으로 디렉터리를 생성함

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
        // [Sentinel] Fixed symlink traversal vulnerability by removing fallback to absolute_path
        if !canonical_path.starts_with(&self.base_dir) {
            return Err(SecurityError::PathTraversal(format!(
                "Path '{}' resolves outside allowed directory. Base: {:?}, Resolved: {:?}",
                user_path, self.base_dir, canonical_path
            )));
        }

        tracing::debug!("Path validation successful: '{:?}'", absolute_path);
        Ok(absolute_path)
    }

    /// Validate a path for read-only operations. Absolute paths outside the base directory are
    /// permitted, while relative paths continue to be constrained to the base directory.
    pub fn validate_path_for_read(&self, user_path: &str) -> Result<PathBuf, SecurityError> {
        let normalized = user_path.replace('\\', "/");

        // Detect Windows drive-letter absolute paths like C:\foo
        let is_windows_absolute = normalized.len() >= 2 && normalized.as_bytes()[1] == b':';

        // Also treat Unix-style absolute paths (starting with '/') as absolute
        let is_unix_style_absolute = normalized.starts_with('/');

        if Path::new(&normalized).is_absolute() || is_windows_absolute || is_unix_style_absolute {
            let cleaned = PathBuf::from(&normalized).clean();
            return Ok(cleaned);
        }

        self.validate_path(&normalized)
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

    /// Normalize path separators to forward slashes for cross-platform compatibility.
    /// This is useful for storing paths in databases or ZIP archives.
    pub fn normalize_path_separators(path: &str) -> String {
        path.replace('\\', "/")
    }

    /// Extract filename from a path, supporting both / and \\ separators.
    /// Returns None if the path is empty or ends with a separator.
    pub fn extract_filename(path: &str) -> Option<String> {
        let normalized = Self::normalize_path_separators(path);
        normalized.split('/').next_back().map(|s| s.to_string())
    }
}

// ========================================
// UI Resource Response Helpers
// ========================================

use crate::mcp::types::{MCPContent, MCPResult, ServiceInfo};
use serde_json::json;

/// Creates a standardized UI resource response with service information.
///
/// This helper ensures all UI resources include proper service metadata
/// for correct routing of user interactions on the frontend.
///
/// # Arguments
/// * `uri` - The resource URI (e.g., "ui://prompt/123")
/// * `mime_type` - The MIME type (typically "text/html")
/// * `html` - The rendered HTML content
/// * `server_name` - The name of the server (e.g., "ui", "playbook")
/// * `tool_name` - The name of the tool (e.g., "promptUser", "visualizeData")
/// * `message` - Optional text message to prepend before the resource
///
/// # Returns
/// An `MCPResult` containing the resource with embedded `ServiceInfo`
pub fn create_resource_response(
    uri: &str,
    mime_type: &str,
    html: &str,
    server_name: &str,
    tool_name: &str,
    message: Option<&str>,
) -> MCPResult {
    let service_info = ServiceInfo {
        server_name: server_name.to_string(),
        tool_name: tool_name.to_string(),
        backend_type: "BuiltInRust".to_string(),
    };

    let mut content = Vec::new();

    // Add text message if provided (for workspace tools)
    if let Some(msg) = message {
        content.push(MCPContent::Text {
            text: msg.to_string(),
            is_error: None,
        });
    }

    // Add resource content
    content.push(MCPContent::Resource {
        resource: json!({
            "uri": uri,
            "mimeType": mime_type,
            "text": html,
        }),
        service_info,
    });

    MCPResult {
        content: Some(content),
        structured_content: None,
        is_error: Some(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_validation() {
        let temp_dir = std::env::temp_dir().join("mcp_test");
        let validator = SecurityValidator::new_with_base_dir(temp_dir.clone());

        // Valid paths
        assert!(validator.validate_path("test.txt").is_ok());
        assert!(validator.validate_path("./test.txt").is_ok());
        assert!(validator.validate_path("subdir/test.txt").is_ok());
        assert!(validator
            .validate_path("attachments/docker_조사....md")
            .is_ok());

        // Absolute paths for read operations should be allowed
        assert!(validator
            .validate_path_for_read("/tmp/some-file.txt")
            .is_ok());

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

    #[test]
    fn test_normalize_path_separators() {
        let windows_path = "C:\\Users\\user\\file.txt";
        let normalized = SecurityValidator::normalize_path_separators(windows_path);
        assert_eq!(normalized, "C:/Users/user/file.txt");

        let mixed_path = "C:/Users\\user/file.txt";
        let normalized = SecurityValidator::normalize_path_separators(mixed_path);
        assert_eq!(normalized, "C:/Users/user/file.txt");

        let unix_path = "/home/user/file.txt";
        let normalized = SecurityValidator::normalize_path_separators(unix_path);
        assert_eq!(normalized, "/home/user/file.txt");
    }

    #[test]
    fn test_extract_filename() {
        // Windows paths
        let path = "C:\\Users\\user\\Downloads\\test.pdf";
        let filename = SecurityValidator::extract_filename(path);
        assert_eq!(filename, Some("test.pdf".to_string()));

        // Unix paths
        let path = "/home/user/downloads/test.pdf";
        let filename = SecurityValidator::extract_filename(path);
        assert_eq!(filename, Some("test.pdf".to_string()));

        // Mixed separators
        let path = "C:/Users/user\\Downloads\\test.pdf";
        let filename = SecurityValidator::extract_filename(path);
        assert_eq!(filename, Some("test.pdf".to_string()));

        // Edge cases
        let path = "test.pdf";
        let filename = SecurityValidator::extract_filename(path);
        assert_eq!(filename, Some("test.pdf".to_string()));

        let path = "";
        let filename = SecurityValidator::extract_filename(path);
        assert_eq!(filename, Some("".to_string()));

        let path = "C:\\Users\\";
        let filename = SecurityValidator::extract_filename(path);
        assert_eq!(filename, Some("".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_traversal_prevention() {
        use std::os::unix::fs::symlink;
        use std::fs::File;
        use std::io::Write;

        let temp_dir = tempfile::tempdir().unwrap();
        let base_dir = temp_dir.path().join("base");
        std::fs::create_dir(&base_dir).unwrap();

        let validator = SecurityValidator::new_with_base_dir(base_dir.clone());

        // Create a file outside the base directory
        let outside_file = temp_dir.path().join("secret.txt");
        let mut file = File::create(&outside_file).unwrap();
        writeln!(file, "secret data").unwrap();

        // Create a symlink inside base directory pointing to outside file
        let symlink_path = base_dir.join("innocent_link");
        symlink(&outside_file, &symlink_path).unwrap();

        // Validate the symlink path - MUST FAIL
        let result = validator.validate_path("innocent_link");

        assert!(result.is_err(), "Symlink pointing outside base directory should be rejected");
        match result {
            Err(SecurityError::PathTraversal(_)) => {}, // Expected
            _ => panic!("Expected PathTraversal error, got {:?}", result),
        }
    }
}
