use super::paths::{normalize_user_path, path_starts_with};
use super::sensitive_path_policy::matches_sensitive_path_policy;
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
    enforce_base_dir_containment: bool,
}

impl SecurityValidator {
    pub fn new_with_base_dir(base_dir: PathBuf) -> Self {
        Self::new_internal(base_dir, false)
    }

    pub fn new_scoped_with_base_dir(base_dir: PathBuf) -> Self {
        Self::new_internal(base_dir, true)
    }

    fn new_internal(base_dir: PathBuf, enforce_base_dir_containment: bool) -> Self {
        tracing::info!(
            "SecurityValidator created with custom base_dir = {:?}, scoped = {}",
            base_dir,
            enforce_base_dir_containment
        );

        if let Err(e) = std::fs::create_dir_all(&base_dir) {
            tracing::error!("Failed to create base directory {:?}: {}", base_dir, e);
        }

        let canonical_base = base_dir.canonicalize().unwrap_or_else(|e| {
            tracing::warn!("Failed to canonicalize base_dir {:?}: {}", base_dir, e);
            base_dir
        });

        Self {
            base_dir: canonical_base,
            enforce_base_dir_containment,
        }
    }

    /// Validate and clean a file path to prevent directory traversal.
    ///
    /// Read-only validation intentionally allows general absolute paths outside
    /// `base_dir` as long as they are not sensitive and do not traverse via
    /// parent segments or unsafe symlinks. Write validation tightens this by
    /// requiring the resolved target to stay within `base_dir`.
    pub fn validate_path(&self, user_path: &str) -> Result<PathBuf, SecurityError> {
        self.validate_path_internal(user_path, self.enforce_base_dir_containment)
    }

    fn validate_path_internal(
        &self,
        user_path: &str,
        enforce_base_containment: bool,
    ) -> Result<PathBuf, SecurityError> {
        // Log the input and effective base directory to simplify security debugging.
        tracing::debug!(
            "Validating path: '{}' against base: '{:?}'",
            user_path,
            self.base_dir
        );

        let prepared_path = normalize_user_path(user_path);
        let clean_path = PathBuf::from(&prepared_path).clean();

        // Intentionally not cfg!(windows)-gated: a drive-letter path must not be joined under
        // base_dir on Unix either (otherwise `C:/...` becomes a relative workspace path).
        let is_win_drive = prepared_path.len() >= 2
            && prepared_path.as_bytes()[0].is_ascii_alphabetic()
            && prepared_path.as_bytes()[1] == b':';

        let absolute_path = if clean_path.is_absolute() || is_win_drive {
            clean_path
        } else {
            self.base_dir.join(clean_path)
        };

        // Block parent-directory traversal before any filesystem access.
        if Path::new(&prepared_path)
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(SecurityError::PathTraversal(format!(
                "Parent directory traversal not allowed: '{user_path}'"
            )));
        }

        // Individual path component length limit.
        // Windows MAX_PATH is 260 chars for the full path; even with long-path
        // extensions enabled, single components > 255 chars are always invalid.
        // Reject them early to avoid triggering OS error 123 deep in the stack.
        const MAX_COMPONENT_LEN: usize = 255;
        for component in Path::new(&prepared_path).components() {
            if let Component::Normal(name) = component {
                if name.len() > MAX_COMPONENT_LEN {
                    return Err(SecurityError::InvalidPath(format!(
                        "Path component '{}...' exceeds maximum length of {} characters",
                        &name.to_string_lossy()[..50],
                        MAX_COMPONENT_LEN
                    )));
                }
            }
        }

        tracing::debug!("Resolved path: '{:?}'", absolute_path);

        // Prefer a canonical path for the early containment check. On macOS, tempfile and
        // file:// URLs often use `/var/folders/...` while `base_dir` is stored as the
        // canonical `/private/var/folders/...` form after construction.
        let early_containment_path = absolute_path
            .canonicalize()
            .unwrap_or_else(|_| absolute_path.clone());

        if enforce_base_containment && !path_starts_with(&early_containment_path, &self.base_dir) {
            return Err(SecurityError::PathTraversal(format!(
                "Access denied: Path '{user_path}' is outside the allowed base directory"
            )));
        }

        self.ensure_not_sensitive_path(&early_containment_path, user_path)?;

        // SecurityValidator only validates paths. Directory creation is handled explicitly by callers.

        // Canonicalize whenever possible to catch symlink-based escapes.
        let canonical_path = match absolute_path.canonicalize() {
            Ok(path) => path,
            Err(_) => {
                // If canonicalize fails, it might be because the target doesn't exist yet.
                // However, check if the exact target path itself is an unresolved symlink.
                // If it is, we must reject it to prevent arbitrary file writes via dangling symlinks.
                if let Ok(metadata) = std::fs::symlink_metadata(&absolute_path) {
                    if metadata.file_type().is_symlink() {
                        return Err(SecurityError::PathTraversal(format!(
                            "Path '{}' is an unresolved symlink: {:?}",
                            user_path, absolute_path
                        )));
                    }
                }

                // The full path may not exist yet during create/write flows, so walk upward until we
                // find an existing parent. Reject unresolved symlink parents instead of skipping them,
                // because a dangling symlink can later resolve outside base_dir.
                let mut current = absolute_path.as_path();
                let mut existing_canonical = None;
                while let Some(parent) = current.parent() {
                    match parent.canonicalize() {
                        Ok(canon) => {
                            existing_canonical = Some(canon);
                            break;
                        }
                        Err(_) => {
                            if let Ok(metadata) = std::fs::symlink_metadata(parent) {
                                if metadata.file_type().is_symlink() {
                                    return Err(SecurityError::PathTraversal(format!(
                                        "Path '{}' contains an unresolved symlink parent: {:?}",
                                        user_path, parent
                                    )));
                                }
                            }
                            current = parent;
                        }
                    }
                }

                if let Some(canon) = existing_canonical {
                    if enforce_base_containment && !path_starts_with(&canon, &self.base_dir) {
                        return Err(SecurityError::PathTraversal(format!(
                            "Access denied: Path '{user_path}' resolves outside the allowed base directory"
                        )));
                    }
                    self.ensure_not_sensitive_path(&canon, user_path)?;
                }

                tracing::debug!(
                    "File doesn't exist yet, using non-canonical path: '{:?}'",
                    absolute_path
                );
                absolute_path.clone()
            }
        };

        if enforce_base_containment && !path_starts_with(&canonical_path, &self.base_dir) {
            return Err(SecurityError::PathTraversal(format!(
                "Access denied: Path '{user_path}' resolves outside the allowed base directory"
            )));
        }

        self.ensure_not_sensitive_path(&canonical_path, user_path)?;

        tracing::debug!("Path validation successful: '{:?}'", absolute_path);
        Ok(absolute_path)
    }

    fn ensure_not_sensitive_path(
        &self,
        candidate_path: &Path,
        user_path: &str,
    ) -> Result<(), SecurityError> {
        if self.is_sensitive_path(candidate_path) {
            return Err(SecurityError::AccessDenied(format!(
                "Path '{}' targets a protected location: '{}'",
                user_path,
                candidate_path.display()
            )));
        }

        Ok(())
    }

    fn is_sensitive_path(&self, candidate_path: &Path) -> bool {
        matches_sensitive_path_policy(candidate_path)
    }

    /// Validate a path for write/create operations.
    ///
    /// Same as [`Self::validate_path`] but additionally blocks Windows reserved filenames
    /// (`CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`).  These names
    /// must be forbidden on creation because once written they become undeletable on
    /// Windows.  Deletion uses plain [`Self::validate_path`] so that pre-existing
    /// reserved-name files can still be cleaned up.
    pub fn validate_path_for_write(&self, user_path: &str) -> Result<PathBuf, SecurityError> {
        static RESERVED: &[&str] = &[
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        let prepared = normalize_user_path(user_path);
        let path_for_check = PathBuf::from(&prepared);
        for component in path_for_check.components() {
            if let Component::Normal(name) = component {
                // Windows strips trailing spaces and dots from path components before resolving
                // device names, so normalize those away before checking reserved names.
                let stem = Path::new(name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .trim_end_matches([' ', '.']);
                let upper = stem.to_uppercase();
                if RESERVED.contains(&upper.as_str()) {
                    return Err(SecurityError::InvalidPath(format!(
                        "Windows reserved filename '{}' is not allowed in path: '{user_path}'",
                        stem
                    )));
                }
            }
        }
        self.validate_path_internal(user_path, self.enforce_base_dir_containment)
    }

    /// Validate a path for read-only operations.
    ///
    pub fn validate_path_for_read(&self, user_path: &str) -> Result<PathBuf, SecurityError> {
        self.validate_path(user_path)
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
        path.replace(['\\', '/'], "/")
    }

    /// Extract filename from a path, supporting both / and \\ separators.
    /// Returns None if the path is empty or ends with a separator.
    pub fn extract_filename(path: &str) -> Option<String> {
        let normalized = Self::normalize_path_separators(path);
        normalized.split('/').next_back().map(|s| s.to_string())
    }
}
