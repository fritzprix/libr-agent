use std::path::{Path, PathBuf};
use tokio::fs;

/// Expand leading home shortcuts to an absolute path.
///
/// Recognizes `~`, `~/...`, `~\...`, `$HOME`, `$HOME/...`, `${HOME}`, and `${HOME}/...`.
/// Other inputs are returned unchanged. This prevents file tools from treating `~` as a
/// literal relative directory name under the workspace root.
pub fn expand_home_pseudo(path: &str) -> PathBuf {
    let path = path.trim();
    if path.is_empty() {
        return PathBuf::new();
    }

    let Some(home) = dirs::home_dir() else {
        return PathBuf::from(path);
    };

    if path == "~" || path == "$HOME" || path == "${HOME}" {
        return home;
    }

    if let Some(rest) = path
        .strip_prefix("~/")
        .or_else(|| path.strip_prefix("~\\"))
        .or_else(|| path.strip_prefix("$HOME/"))
        .or_else(|| path.strip_prefix("$HOME\\"))
        .or_else(|| path.strip_prefix("${HOME}/"))
        .or_else(|| path.strip_prefix("${HOME}\\"))
    {
        return home.join(rest);
    }

    PathBuf::from(path)
}

/// True when `path` uses a leading home shortcut that [`expand_home_pseudo`] understands.
pub fn is_home_pseudo_path(path: &str) -> bool {
    let path = path.trim();
    path == "~"
        || path.starts_with("~/")
        || path.starts_with("~\\")
        || path == "$HOME"
        || path.starts_with("$HOME/")
        || path.starts_with("$HOME\\")
        || path == "${HOME}"
        || path.starts_with("${HOME}/")
        || path.starts_with("${HOME}\\")
}

/// Resolves a relative path within a base directory, ensuring it stays within the base.
///
/// This function performs the following steps:
/// 1. Canonicalizes the base directory to ensure a stable root.
/// 2. Joins the relative path to the base, ensuring it's treated as relative.
/// 3. Canonicalizes the resulting path to resolve symlinks and `..` components.
/// 4. Checks if the resulting path starts with the canonical base.
///
/// Note: The target file or directory MUST exist for this validation to succeed,
/// as `canonicalize` requires existence.
///
/// # Arguments
/// * `base_dir` - The base directory to restrict access to.
/// * `relative_path` - The relative path to resolve.
///
/// # Returns
/// * `Ok(PathBuf)` - The canonicalized absolute path if valid and safe.
/// * `Err(String)` - Error message if path is invalid or outside base.
pub async fn resolve_secure_path(base_dir: &Path, relative_path: &str) -> Result<PathBuf, String> {
    // 1. Canonicalize base_dir
    let canonical_base = fs::canonicalize(base_dir)
        .await
        .map_err(|e| format!("Failed to canonicalize base dir: {e}"))?;

    // 2. Prevent absolute paths in relative_path from bypassing the join.
    // We treat the input as strictly relative to the base.
    if Path::new(relative_path).is_absolute() {
        return Err("Absolute paths are not allowed in workspace-scoped operations".to_string());
    }

    // Fallback: Remove leading separators in case the path is not strictly considered absolute
    // but starts with a separator (e.g., on Windows).
    let safe_relative = relative_path.trim_start_matches(std::path::is_separator);

    // Reject Windows drive letters (e.g. "C:...") only on Windows, and only when the
    // first character is an ASCII alphabetic drive letter.
    if cfg!(windows)
        && safe_relative.len() >= 2
        && safe_relative.as_bytes()[0].is_ascii_alphabetic()
        && safe_relative.as_bytes()[1] == b':'
    {
        return Err("Absolute paths with drive letters are not allowed".to_string());
    }

    let full_path = canonical_base.join(safe_relative);

    // 3. Canonicalize target
    // Note: The target file/directory must exist for canonicalize to work.
    let canonical_target = fs::canonicalize(&full_path)
        .await
        .map_err(|e| format!("File not found or invalid path: {}", e))?;

    // 4. Verify containment
    if !canonical_target.starts_with(&canonical_base) {
        return Err("Access denied: Path is outside workspace".to_string());
    }

    Ok(canonical_target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_expand_home_pseudo_shortcuts() {
        let Some(home) = dirs::home_dir() else {
            return;
        };
        assert_eq!(expand_home_pseudo("~"), home);
        assert_eq!(expand_home_pseudo("~/a/b"), home.join("a/b"));
        assert_eq!(expand_home_pseudo("$HOME/a"), home.join("a"));
        assert_eq!(expand_home_pseudo("${HOME}/a"), home.join("a"));
        assert_eq!(expand_home_pseudo("~backup"), PathBuf::from("~backup"));
        assert_eq!(expand_home_pseudo("src/main.rs"), PathBuf::from("src/main.rs"));
        assert!(is_home_pseudo_path("~/docs"));
        assert!(!is_home_pseudo_path("~docs"));
        assert!(!is_home_pseudo_path("./~/docs"));
    }

    #[tokio::test]
    async fn test_resolve_secure_path_valid() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        File::create(&file_path).unwrap();

        let result = resolve_secure_path(dir.path(), "test.txt").await;
        assert!(result.is_ok());

        let resolved = result.unwrap();
        let expected = fs::canonicalize(&file_path).await.unwrap();
        assert_eq!(resolved, expected);
    }

    #[tokio::test]
    async fn test_resolve_secure_path_traversal_attempt() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("subdir");
        std::fs::create_dir(&subdir).unwrap();

        // Create file in root (outside subdir)
        let outside_file = dir.path().join("outside.txt");
        File::create(&outside_file).unwrap();

        // Try to access it from subdir using ..
        let result = resolve_secure_path(&subdir, "../outside.txt").await;

        // Should fail because resolved path is outside base (subdir)
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Access denied"));
    }

    #[tokio::test]
    async fn test_resolve_secure_path_absolute_as_relative() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        File::create(&file_path).unwrap();

        // Test logic: absolute paths should be rejected.
        let result = resolve_secure_path(dir.path(), "/test.txt").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Absolute paths are not allowed in workspace-scoped operations"));

        let abs_path = file_path.to_str().unwrap();
        let result_abs = resolve_secure_path(dir.path(), abs_path).await;
        assert!(result_abs.is_err());
    }

    #[tokio::test]
    async fn test_resolve_secure_path_nonexistent() {
        let dir = tempdir().unwrap();
        let result = resolve_secure_path(dir.path(), "nonexistent.txt").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("File not found"));
    }

    #[tokio::test]
    #[cfg(not(windows))]
    async fn test_resolve_secure_path_unix_filename_with_colon() {
        // On Unix, filenames can contain colons (e.g., "a:b")
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("a:b");
        File::create(&file_path).unwrap();

        let result = resolve_secure_path(dir.path(), "a:b").await;
        assert!(
            result.is_ok(),
            "Unix filename with colon should be accepted"
        );

        let resolved = result.unwrap();
        let expected = fs::canonicalize(&file_path).await.unwrap();
        assert_eq!(resolved, expected);
    }
}
