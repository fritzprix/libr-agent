/// Integration tests for SecureFileManager.
///
/// These live in `tests/` so they actually run under `cargo test` even though
/// `[lib] test = false` is set in Cargo.toml.
use std::sync::{Mutex, MutexGuard, OnceLock};
use tauri_mcp_agent_lib::SecureFileManager;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// RAII env-var guard
// ---------------------------------------------------------------------------

/// Global mutex serialises mutations of LIBRAGENT_MAX_FILE_SIZE across tests
/// to prevent races when tests run in parallel.
static MAX_FILE_SIZE_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn max_file_size_env_mutex() -> &'static Mutex<()> {
    MAX_FILE_SIZE_ENV_LOCK.get_or_init(|| Mutex::new(()))
}

/// RAII guard: temporarily sets an env var and restores it on `Drop`.
struct EnvVarGuard {
    name: String,
    prev_value: Option<String>,
    // Held for the lifetime of the guard to serialise access.
    _lock: MutexGuard<'static, ()>,
}

impl EnvVarGuard {
    fn set_temp(name: &str, value: &str) -> Self {
        let lock = max_file_size_env_mutex()
            .lock()
            .expect("env mutex poisoned");
        let prev_value = std::env::var(name).ok();
        // SAFETY: single-threaded access is serialised by the mutex above.
        unsafe { std::env::set_var(name, value) };
        Self {
            name: name.to_string(),
            prev_value,
            _lock: lock,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // SAFETY: single-threaded access is serialised by the mutex in _lock.
        unsafe {
            if let Some(ref v) = self.prev_value {
                std::env::set_var(&self.name, v);
            } else {
                std::env::remove_var(&self.name);
            }
        }
        // _lock is dropped here, releasing the mutex.
    }
}

// ---------------------------------------------------------------------------
// Regression test: append file size limit bypass (PR #564)
// ---------------------------------------------------------------------------

/// Regression test for the file-size limit bypass in `append_file_string`.
///
/// Before the fix, repeatedly appending small chunks could grow a file past
/// `LIBRAGENT_MAX_FILE_SIZE` because the projected size was never checked.
#[tokio::test]
async fn test_append_file_string_limit_bypass() {
    // Set a small limit (10 bytes) for the duration of this test.
    let _env_guard = EnvVarGuard::set_temp("LIBRAGENT_MAX_FILE_SIZE", "10");

    let dir = tempdir().unwrap();
    let manager = SecureFileManager::new_with_base_dir(dir.path().to_path_buf());

    // Create a file with 5 bytes — within the limit.
    manager
        .write_file_string("test.txt", "12345")
        .await
        .unwrap();

    // Attempt to append 6 bytes — total would be 11, which exceeds the 10-byte limit.
    let result: Result<(), String> = manager.append_file_string("test.txt", "123456").await;

    assert!(
        result.is_err(),
        "Append should fail when resulting file size would exceed the configured limit"
    );
}

/// Appending within the limit should succeed.
#[tokio::test]
async fn test_append_file_string_within_limit() {
    let _env_guard = EnvVarGuard::set_temp("LIBRAGENT_MAX_FILE_SIZE", "20");

    let dir = tempdir().unwrap();
    let manager = SecureFileManager::new_with_base_dir(dir.path().to_path_buf());

    manager
        .write_file_string("test.txt", "12345")
        .await
        .unwrap();

    // 5 + 5 = 10, well within limit of 20.
    let result: Result<(), String> = manager.append_file_string("test.txt", "67890").await;

    assert!(
        result.is_ok(),
        "Append should succeed when within the configured limit"
    );
}

fn assert_windows_reserved_filename_error(result: Result<(), String>, path: &str) {
    let error = result.expect_err("Operation should reject Windows reserved filenames");
    assert!(
        error.contains("Windows reserved filename"),
        "Expected reserved filename error for '{path}', got: {error}"
    );
}

fn windows_reserved_filename_bypass_cases() -> [&'static str; 3] {
    ["CON ", "NUL.", "COM1..."]
}

#[tokio::test]
async fn test_write_operations_reject_windows_reserved_filenames() {
    let dir = tempdir().unwrap();
    let manager = SecureFileManager::new_with_base_dir(dir.path().to_path_buf());

    assert_windows_reserved_filename_error(manager.write_file("CON", b"blocked").await, "CON");
    assert_windows_reserved_filename_error(
        manager.write_file_string("NUL.txt", "blocked").await,
        "NUL.txt",
    );
    assert_windows_reserved_filename_error(
        manager.append_file_string("COM1.txt", "blocked").await,
        "COM1.txt",
    );

    for path in windows_reserved_filename_bypass_cases() {
        assert_windows_reserved_filename_error(manager.write_file(path, b"blocked").await, path);
    }
}

#[tokio::test]
async fn test_copy_file_from_external_rejects_windows_reserved_filenames() {
    let dir = tempdir().unwrap();
    let manager = SecureFileManager::new_with_base_dir(dir.path().to_path_buf());

    let external_source = dir.path().join("source.txt");
    std::fs::write(&external_source, "blocked").unwrap();

    let error = manager
        .copy_file_from_external(&external_source, "LPT1.txt")
        .await
        .expect_err("Copy should reject Windows reserved filenames");

    assert!(
        error.contains("Windows reserved filename"),
        "Expected reserved filename error for copy destination, got: {error}"
    );

    for path in windows_reserved_filename_bypass_cases() {
        let error = manager
            .copy_file_from_external(&external_source, path)
            .await
            .expect_err("Copy should reject Windows reserved filename bypass variants");

        assert!(
            error.contains("Windows reserved filename"),
            "Expected reserved filename error for copy destination '{path}', got: {error}"
        );
    }
}

#[tokio::test]
async fn test_external_paths_are_allowed_and_sensitive_targets_are_scoped() {
    let workspace = tempdir().unwrap();
    let external_root = tempdir().unwrap();
    let external_file = external_root.path().join("external.txt");

    let manager = SecureFileManager::new_with_base_dir(workspace.path().to_path_buf());

    manager
        .write_file_string(&external_file.to_string_lossy(), "allowed")
        .await
        .expect("general external writes should be allowed");

    let content = manager
        .read_file_as_string(&external_file.to_string_lossy())
        .await
        .expect("general external reads should be allowed");
    assert_eq!(content, "allowed");

    let project_env_path = external_root.path().join(".env.local");
    manager
        .write_file_string(&project_env_path.to_string_lossy(), "still allowed")
        .await
        .expect("project-local .env outside home should remain allowed");

    let sensitive_path = dirs::home_dir().expect("home dir").join(".env.local");
    let error = manager
        .write_file_string(&sensitive_path.to_string_lossy(), "blocked")
        .await
        .expect_err("sensitive paths should stay blocked");
    assert!(
        error.contains("protected location"),
        "unexpected sensitive path error: {error}"
    );
}
