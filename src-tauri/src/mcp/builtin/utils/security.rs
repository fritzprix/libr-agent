use super::paths::{normalize_user_path, path_starts_with};
use super::sensitive_path_policy::matches_sensitive_path_policy;
use path_clean::PathClean;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

pub const OUTSIDE_WORKSPACE_ROOT_MARKER: &str = "Outside workspace root:";

#[derive(Error, Debug)]
pub enum SecurityError {
    #[error("Path traversal attempt detected: {0}")]
    PathTraversal(String),
    #[error("Outside workspace root: {0}")]
    OutsideWorkspaceRoot(String),
    #[error("Access denied: {0}")]
    #[allow(dead_code)]
    AccessDenied(String),
    #[error("File size limit exceeded: {0} bytes")]
    FileSizeLimit(usize),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

/// True when a relative `user_path` still climbs out of its start after lexical
/// normalization.
///
/// `a/../b` resolves to `b` and is fine; `../b` and `a/../../b` are not. A rooted path
/// cannot climb above its root (`/x/../../b` normalizes to `/b`), so for those this
/// returns `false` and containment is enforced by `validate_path_internal`, not here.
/// Normalization is lexical: `link/../x` with a symlink `link` validates as `x`, where a
/// shell would resolve it through the link target. File I/O uses the validated path.
pub fn has_parent_traversal(user_path: &str) -> bool {
    PathBuf::from(normalize_user_path(user_path))
        .clean()
        .components()
        .any(|component| matches!(component, Component::ParentDir))
}

/// `path` with its longest existing prefix canonicalized, so symlinks in existing
/// parents are resolved for a target that does not exist yet.
fn canonicalize_existing_prefix(path: &Path) -> PathBuf {
    let mut missing = Vec::new();
    let mut current = path;
    loop {
        if let Ok(canonical) = current.canonicalize() {
            return missing
                .iter()
                .rev()
                .fold(canonical, |acc, name| acc.join(name));
        }
        match (current.parent(), current.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name.to_os_string());
                current = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// True when a component is `.git`. Case, trailing dots/spaces, and an NTFS stream
/// suffix (`.git::$INDEX_ALLOCATION`) are ignored because Windows and macOS resolve
/// those variants to the same entry.
fn has_git_component(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => name.to_str().is_some_and(|name| {
            let name = name.split(':').next().unwrap_or(name);
            name.trim_end_matches([' ', '.'])
                .eq_ignore_ascii_case(".git")
        }),
        _ => false,
    })
}

/// Security utilities for built-in servers
pub struct SecurityValidator {
    base_dir: PathBuf,
    /// Canonical directories accepted in addition to `base_dir` by the containment
    /// checks (linked git worktrees of the workspace repository). Relative paths still
    /// resolve against `base_dir`.
    extra_roots: Vec<PathBuf>,
    enforce_base_dir_containment: bool,
    /// When set, writes into git metadata are refused: any path with a `.git`
    /// component, or a path under one of these canonical git dirs.
    git_write_guard: Option<Vec<PathBuf>>,
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
            extra_roots: Vec::new(),
            enforce_base_dir_containment,
            git_write_guard: None,
        }
    }

    /// Refuse writes into git metadata (`.git` dirs and files, and `git_dirs`, normally the
    /// common git dir of the workspace repository). Reads are not affected. Dirs that
    /// cannot be canonicalized are dropped; the `.git` component check still applies.
    pub fn with_git_metadata_write_guard(mut self, git_dirs: Vec<PathBuf>) -> Self {
        self.git_write_guard = Some(
            git_dirs
                .into_iter()
                .filter_map(|dir| dir.canonicalize().ok())
                .collect(),
        );
        self
    }

    /// Also accept paths under `roots` in the containment checks. Roots that cannot be
    /// canonicalized are dropped.
    pub fn with_extra_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.extra_roots = roots
            .into_iter()
            .filter_map(|root| root.canonicalize().ok())
            .collect();
        self
    }

    fn is_within_allowed_roots(&self, path: &Path) -> bool {
        path_starts_with(path, &self.base_dir)
            || self
                .extra_roots
                .iter()
                .any(|root| path_starts_with(path, root))
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
        if has_parent_traversal(user_path) {
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

        // Only paths whose raw and canonical forms both lie outside `base_dir` are direct
        // external paths (`OutsideWorkspaceRoot`); paths inside `base_dir` that resolve outside
        // via symlinks are rejected as `PathTraversal` by the canonical checks below.
        if enforce_base_containment
            && !self.is_within_allowed_roots(&absolute_path)
            && !self.is_within_allowed_roots(&early_containment_path)
        {
            return Err(SecurityError::OutsideWorkspaceRoot(format!(
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
                    if enforce_base_containment && !self.is_within_allowed_roots(&canon) {
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

        if enforce_base_containment && !self.is_within_allowed_roots(&canonical_path) {
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
        let path = self.validate_path_internal(user_path, self.enforce_base_dir_containment)?;
        self.ensure_not_git_metadata(&path, user_path)?;
        Ok(path)
    }

    fn ensure_not_git_metadata(&self, path: &Path, user_path: &str) -> Result<(), SecurityError> {
        let Some(git_dirs) = &self.git_write_guard else {
            return Ok(());
        };
        let resolved = canonicalize_existing_prefix(path);
        let in_git_metadata = |candidate: &Path| {
            has_git_component(candidate)
                || git_dirs.iter().any(|dir| path_starts_with(candidate, dir))
        };
        if in_git_metadata(path) || in_git_metadata(&resolved) {
            return Err(SecurityError::AccessDenied(format!(
                "Path '{user_path}' is inside git metadata ('{}'). Writes into git metadata \
                 (.git directories and files) are not allowed.",
                resolved.display()
            )));
        }
        Ok(())
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

#[cfg(test)]
mod parent_traversal_tests {
    use super::{has_parent_traversal, SecurityError};
    use crate::mcp::builtin::utils::linked_worktrees::test_support::{add_worktree, main_repo};
    use crate::mcp::builtin::workspace::workspace_server::file_tool_manager;
    use tempfile::tempdir;

    #[test]
    fn normalizes_before_rejecting_parent_segments() {
        assert!(!has_parent_traversal("src/../lib/a.rs"));
        assert!(!has_parent_traversal("/repo/../repo.worktrees/w/a.rs"));
        assert!(has_parent_traversal("../a.rs"));
        assert!(has_parent_traversal("src/../../a.rs"));
        // Rooted paths cannot climb above the root; containment handles them.
        assert!(!has_parent_traversal("/x/../../b"));
    }

    #[test]
    fn scoped_validator_accepts_a_normalized_sibling_worktree_path() {
        let parent = tempdir().expect("tempdir");
        let repo = main_repo(parent.path());
        add_worktree(&repo, "w");
        let manager = file_tool_manager(repo.clone(), true);
        let validator = manager.get_security_validator();
        // Callers pass a plain drive path (`git rev-parse`), not a `\?\` verbatim one.
        let base = repo.display().to_string();
        let base = crate::mcp::builtin::utils::strip_windows_verbatim_prefix(&base);

        let accepted = validator
            .validate_path_for_write(&format!("{base}/../repo.worktrees/w/a.rs"))
            .expect("normalized sibling worktree path is allowed");
        assert!(accepted.ends_with("a.rs"));

        let outside = validator
            .validate_path_for_write(&format!("{base}/../outside/a.rs"))
            .expect_err("path outside every allowed root is rejected");
        assert!(matches!(outside, SecurityError::OutsideWorkspaceRoot(_)));

        let climbing = validator
            .validate_path_for_write("a/../../x")
            .expect_err("relative path that climbs out is rejected");
        assert!(matches!(climbing, SecurityError::PathTraversal(_)));
    }
}
