use tokio::fs;
use tokio::io::AsyncReadExt;
use tracing::{error, info};

use crate::mcp::builtin::utils::SecurityValidator;

/// Phrase in the error for a parent path that runs through an existing file.
/// `writeFile` keys its recovery hint on it.
pub const ANCESTOR_IS_FILE: &str = "is an existing file, not a directory";

/// English, locale-independent error for a failed `create_dir_all(parent)`.
///
/// The OS error text follows the host locale (Korean on a Korean host), so it
/// goes to the log only. When the first existing ancestor of `parent` is a
/// regular file, the message names that file, because no permission change can
/// turn it into a directory.
fn parent_dir_error(parent: &std::path::Path, err: &std::io::Error) -> String {
    error!("Failed to create parent directory {:?}: {}", parent, err);
    let mut ancestor = Some(parent);
    while let Some(dir) = ancestor {
        if dir.exists() {
            if dir.is_file() {
                return format!(
                    "Cannot create '{}': '{}' {ANCESTOR_IS_FILE}. Choose a path whose parent is a directory.",
                    display_path(parent),
                    display_path(dir)
                );
            }
            break;
        }
        ancestor = dir.parent();
    }
    format!(
        "Failed to create parent directory '{}': {}",
        display_path(parent),
        err.kind()
    )
}

/// Path text for messages, without the Windows verbatim prefix (`\\?\`) that
/// canonicalized paths carry.
fn display_path(path: &std::path::Path) -> String {
    let text = path.display().to_string();
    text.strip_prefix(r"\\?\")
        .map(str::to_string)
        .unwrap_or(text)
}

/// Provides secure file system operations with path validation and optional
/// base-directory scoping depending on how the manager is constructed.
pub struct SecureFileManager {
    security: SecurityValidator,
}

impl SecureFileManager {
    /// Creates a new `SecureFileManager` with a specified base directory.
    ///
    /// # Arguments
    /// * `base_dir` - The anchor used for resolving relative paths.
    pub fn new_with_base_dir(base_dir: std::path::PathBuf) -> Self {
        Self {
            security: SecurityValidator::new_with_base_dir(base_dir),
        }
    }

    /// Creates a `SecureFileManager` that constrains both reads and writes to `base_dir`.
    pub fn new_scoped_with_base_dir(base_dir: std::path::PathBuf) -> Self {
        Self {
            security: SecurityValidator::new_scoped_with_base_dir(base_dir),
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
            .validate_path_for_read(path)
            .map_err(|e| format!("Security error: {e}"))?;

        // Check if file exists and is a file
        if !safe_path.exists() {
            return Err(format!("File does not exist: {path}"));
        }

        if !safe_path.is_file() {
            return Err(format!("Path is not a file: {path}"));
        }

        // Check file size (metadata check)
        let max_size = crate::config::max_file_size();
        if let Err(e) = self.security.validate_file_size(&safe_path, max_size) {
            return Err(format!("File size error: {e}"));
        }

        // Read the file contents with a size limit to prevent TOCTOU/DoS
        let file = fs::File::open(&safe_path)
            .await
            .map_err(|e| format!("Failed to open file: {e}"))?;

        let mut content = Vec::new();
        let read_limit = (max_size as u64).saturating_add(1);
        let bytes_read = file
            .take(read_limit)
            .read_to_end(&mut content)
            .await
            .map_err(|e| format!("Failed to read file: {e}"))?;

        if bytes_read > max_size {
            return Err(format!(
                "File exceeds the maximum allowed size of {} bytes",
                max_size
            ));
        }

        Ok(content)
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
            .validate_path_for_write(path)
            .map_err(|e| format!("Security error: {e}"))?;

        // Check content size
        let max_size = crate::config::max_file_size();
        if content.len() > max_size {
            return Err(format!(
                "Content too large: {} bytes (max: {} bytes)",
                content.len(),
                max_size
            ));
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = safe_path.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                return Err(parent_dir_error(parent, &e));
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
            .validate_path_for_read(path)
            .map_err(|e| format!("Security error: {e}"))?;

        // Check if file exists and is a file
        if !safe_path.exists() {
            return Err(format!("File does not exist: {path}"));
        }

        if !safe_path.is_file() {
            return Err(format!("Path is not a file: {path}"));
        }

        // Check file size (metadata check)
        let max_size = crate::config::max_file_size();
        if let Err(e) = self.security.validate_file_size(&safe_path, max_size) {
            return Err(format!("File size error: {e}"));
        }

        // Read the file contents with a size limit to prevent TOCTOU/DoS
        let file = fs::File::open(&safe_path)
            .await
            .map_err(|e| format!("Failed to open file: {e}"))?;

        let mut buffer = Vec::new();
        let read_limit = (max_size as u64).saturating_add(1);
        let bytes_read = file
            .take(read_limit)
            .read_to_end(&mut buffer)
            .await
            .map_err(|e| format!("Failed to read file: {e}"))?;

        if bytes_read > max_size {
            return Err(format!(
                "File exceeds the maximum allowed size of {} bytes",
                max_size
            ));
        }

        String::from_utf8(buffer).map_err(|_| {
            "Failed to read file: Content appears to be binary or contains invalid UTF-8 characters"
                .to_string()
        })
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
            .validate_path_for_write(path)
            .map_err(|e| format!("Security error: {e}"))?;

        // If the file does not exist, create it by writing the content.
        if !safe_path.exists() {
            return self.write_file_string(path, content).await;
        }

        // Open the existing file and append content
        use tokio::io::AsyncWriteExt;

        // Security check: verify that appending won't exceed file size limit.
        // Keep arithmetic in u64 to avoid truncation on 32-bit targets.
        let current_size: u64 = fs::metadata(&safe_path)
            .await
            .map_err(|e| format!("Failed to get file metadata: {e}"))?
            .len();

        let max_size = crate::config::max_file_size() as u64;
        let append_size = content.len() as u64;

        if current_size.saturating_add(append_size) > max_size {
            return Err(format!(
                "File exceeds the maximum allowed size of {} bytes (current: {}, append: {})",
                max_size, current_size, append_size
            ));
        }

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
            .validate_path_for_write(dest_rel_path)
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
        if let Err(e) = self
            .security
            .validate_file_size(src_path, crate::config::max_file_size())
        {
            return Err(format!("Source file size error: {e}"));
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = dest_path.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                return Err(parent_dir_error(parent, &e));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_names_file_segment(message: &str, file: &std::path::Path) {
        // The validator canonicalizes (\\?\ on Windows, /private/var on macOS).
        let named = display_path(&std::fs::canonicalize(file).unwrap());
        assert!(message.contains(ANCESTOR_IS_FILE), "{message}");
        assert!(message.contains(&named), "{message}");
        assert!(!message.contains(r"\\?\"), "{message}");
        assert!(!message.contains("os error"), "{message}");
    }

    #[tokio::test]
    async fn write_file_under_an_existing_file_names_that_file() {
        let root = tempfile::tempdir().unwrap();
        let blocker = root.path().join("server.py");
        std::fs::write(&blocker, "print(1)\n").unwrap();
        let manager = SecureFileManager::new_scoped_with_base_dir(root.path().to_path_buf());

        let message = manager
            .write_file("server.py/config.json", b"{}")
            .await
            .unwrap_err();
        assert_names_file_segment(&message, &blocker);
    }

    #[tokio::test]
    async fn copy_file_under_an_existing_file_names_that_file() {
        let root = tempfile::tempdir().unwrap();
        let blocker = root.path().join("server.py");
        std::fs::write(&blocker, "print(1)\n").unwrap();
        let source = root.path().join("source.txt");
        std::fs::write(&source, "data").unwrap();
        let manager = SecureFileManager::new_scoped_with_base_dir(root.path().to_path_buf());

        let message = manager
            .copy_file_from_external(&source, "server.py/nested/copy.txt")
            .await
            .unwrap_err();
        assert_names_file_segment(&message, &blocker);
    }

    /// The OS text follows the host locale; the message must not carry it.
    #[test]
    fn other_failures_use_english_kind_text_not_the_localized_os_text() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("missing").join("dir");
        let localized = std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "\u{AD8C}\u{D55C}\u{C774} \u{AC70}\u{BD80}\u{B418}\u{C5C8}\u{C2B5}\u{B2C8}\u{B2E4}",
        );

        let message = parent_dir_error(&parent, &localized);
        assert!(message.contains(&parent.display().to_string()), "{message}");
        assert!(message.contains("permission denied"), "{message}");
        assert!(!message.contains('\u{AD8C}'), "{message}");
    }
}
