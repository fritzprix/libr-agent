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
