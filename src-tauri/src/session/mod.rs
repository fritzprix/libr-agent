pub mod manager;
pub mod types;

pub use manager::*;
pub use types::*;

use log::error;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

static SESSION_MANAGER: OnceLock<SessionManager> = OnceLock::new();

pub fn get_session_manager() -> Result<&'static SessionManager, String> {
    SESSION_MANAGER.get_or_init(|| {
        SessionManager::new().unwrap_or_else(|e| {
            error!("Failed to initialize SessionManager: {e}");
            // Create fallback session manager with temp directory
            let temp_base = std::env::temp_dir().join("com.fritzprix.libragent");
            let _ = std::fs::create_dir_all(temp_base.join("workspaces").join("default"));
            let _ = std::fs::create_dir_all(temp_base.join("workspaces").join("templates"));
            let _ = std::fs::create_dir_all(temp_base.join("logs"));
            let _ = std::fs::create_dir_all(temp_base.join("config"));

            SessionManager {
                base_data_dir: temp_base,
                workspace_pool: Arc::new(RwLock::new(HashMap::new())),
                template_workspace: Arc::new(RwLock::new(None)),
            }
        })
    });
    Ok(SESSION_MANAGER.get().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn test_session_isolation() {
        // Setup temp dir for base
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let base_path = temp_dir.path().to_path_buf();

        // Initialize SessionManager manually
        let session_manager =
            SessionManager::new_with_base_dir(base_path.clone()).expect("Failed to init");

        // Get two distinct session paths
        let session_id_a = "session_a";
        let session_id_b = "session_b";
        let path_a = session_manager.get_session_workspace_dir_by_id(session_id_a);
        let path_b = session_manager.get_session_workspace_dir_by_id(session_id_b);

        // Verify paths are different and correctly structured
        assert_ne!(path_a, path_b);
        // Use path-aware checks that work across Windows/Unix
        assert!(path_a.ends_with(Path::new("workspaces").join("session_a")));
        assert!(path_b.ends_with(Path::new("workspaces").join("session_b")));

        // Create a file in Session A
        let file_a = path_a.join("test.txt");
        fs::write(&file_a, "Hello A").expect("Failed to write file A");

        // Verify it exists in A but NOT in B path
        assert!(file_a.exists());
        let file_b_location = path_b.join("test.txt");
        assert!(!file_b_location.exists());

        // Create file in Session B
        let file_b = path_b.join("other.txt");
        fs::write(&file_b, "Hello B").expect("Failed to write file B");

        assert!(file_b.exists());
        let file_a_location = path_a.join("other.txt");
        assert!(!file_a_location.exists());
    }

    #[test]
    fn test_default_fallback_session_path() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let base_path = temp_dir.path().to_path_buf();

        let session_manager =
            SessionManager::new_with_base_dir(base_path.clone()).expect("Failed to init");

        // If something requests "default" explicitly
        let path_default = session_manager.get_session_workspace_dir_by_id("default");
        assert!(path_default
            .to_string_lossy()
            .contains("workspaces/default"));
        assert!(path_default.exists());
    }

    #[test]
    fn test_path_traversal_prevention() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let base_path = temp_dir.path().to_path_buf();
        let session_manager =
            SessionManager::new_with_base_dir(base_path.clone()).expect("Failed to init");

        // Attempt path traversal with ".." and "/"
        // This should be sanitized to "______vulnerable"
        let malicious_id = "../../vulnerable";
        let safe_path = session_manager.get_session_workspace_dir_by_id(malicious_id);

        let workspaces_dir = base_path.join("workspaces");

        // Verify the path is strictly within workspaces
        assert!(safe_path.starts_with(&workspaces_dir));

        // Verify the sanitized name
        // ".." -> "__" (dot is not alphanumeric)
        // "/" -> "_"
        // So "../../vulnerable" -> "______vulnerable"
        let path_str = safe_path.to_string_lossy();
        assert!(path_str.contains("______vulnerable"));
        assert!(!path_str.contains(".."));

        // Verify another case with weird characters
        let weird_id = "foo/bar\\baz@qux";
        let safe_weird_path = session_manager.get_session_workspace_dir_by_id(weird_id);

        // "foo/bar\baz@qux" -> "foo_bar_baz_qux" (assuming \ is treated as char, wait backslash is replaced too? No, backslash is not alphanumeric)
        // sanitize_session_id replaces non-alphanumeric with _
        // so / -> _
        // \ -> _
        // @ -> _
        // so foo_bar_baz_qux

        let weird_path_str = safe_weird_path.to_string_lossy();
        assert!(weird_path_str.contains("foo_bar_baz_qux"));
    }
}
