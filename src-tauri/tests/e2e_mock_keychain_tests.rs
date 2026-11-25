//! Standalone tests for the debug-only mock keychain backend (#2904).
//!
//! Covers:
//! - `LIBRAGENT_E2E_MOCK_KEYCHAIN=succeed`: set/get/delete persistence and JSONL
//!   invocation logging without leaking secret values.
//! - `LIBRAGENT_E2E_MOCK_KEYCHAIN=fail-write`: `available()` stays true while
//!   `set()` returns an error (2565 M-7 / 2672 M-4).
//! - `LIBRAGENT_E2E_MOCK_KEYCHAIN=unavailable`: `available()` returns false so
//!   `LayeredSecretStore` falls back to `FileSecretStore` (2672 M-5).
//! - Compile-time `#[cfg(debug_assertions)]` gating pin so release builds never
//!   compile the mock keychain in.

use std::sync::Mutex;
use tauri_mcp_agent_lib::mcp::keychain::KeychainSecretStore;
use tauri_mcp_agent_lib::mcp::secret_templates::SecretStore;
use tempfile::tempdir;

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard {
    saved: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
    fn set(vars: &[(&'static str, Option<&str>)]) -> Self {
        let mut saved = Vec::with_capacity(vars.len());
        for &(key, val) in vars {
            saved.push((key, std::env::var(key).ok()));
            match val {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
        Self { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, prev) in &self.saved {
            match prev {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
    }
}

#[tokio::test]
async fn mock_keychain_succeed_stores_reads_deletes_and_logs_without_secret() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().expect("temp dir");
    let dir_str = dir.path().to_str().expect("utf8 path");
    let _env = EnvGuard::set(&[
        ("LIBRAGENT_E2E_CONTROL", Some("1")),
        ("LIBRAGENT_E2E_MOCK_KEYCHAIN", Some("succeed")),
        ("LIBRAGENT_DATA_DIR", Some(dir_str)),
        ("LIBRAGENT_E2E_MOCK_KEYCHAIN_LOG", None),
    ]);

    let store = KeychainSecretStore;
    assert!(store.available());

    let secret_val = "synthetic-super-secret-value-2904";
    assert!(store.set("srv-1", "API_KEY", secret_val).await.is_ok());
    assert_eq!(
        store.get("srv-1", "API_KEY").await.expect("get"),
        Some(secret_val.to_string())
    );
    store.delete("srv-1", "API_KEY").await.expect("delete");
    assert_eq!(
        store
            .get("srv-1", "API_KEY")
            .await
            .expect("get after delete"),
        None
    );

    let log_path = dir.path().join("e2e_mock_keychain.log");
    let log_text = std::fs::read_to_string(&log_path).expect("mock keychain log");
    assert!(log_text.contains("\"op\":\"available\""));
    assert!(log_text.contains("\"op\":\"set\""));
    assert!(log_text.contains("\"op\":\"get\""));
    assert!(log_text.contains("\"op\":\"delete\""));
    assert!(
        !log_text.contains(secret_val),
        "mock keychain log must never contain secret values"
    );
}

#[tokio::test]
async fn mock_keychain_fail_write_reports_available_but_errors_on_set() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().expect("temp dir");
    let dir_str = dir.path().to_str().expect("utf8 path");
    let _env = EnvGuard::set(&[
        ("LIBRAGENT_E2E_CONTROL", Some("1")),
        ("LIBRAGENT_E2E_MOCK_KEYCHAIN", Some("fail-write")),
        ("LIBRAGENT_DATA_DIR", Some(dir_str)),
        ("LIBRAGENT_E2E_MOCK_KEYCHAIN_LOG", None),
    ]);

    let store = KeychainSecretStore;
    assert!(store.available());
    let err = store
        .set("srv-1", "API_KEY", "should-fail")
        .await
        .expect_err("fail-write must error on set");
    assert!(err.contains("fail-write"), "unexpected error: {err}");

    let log_text = std::fs::read_to_string(dir.path().join("e2e_mock_keychain.log")).expect("log");
    assert!(log_text.contains("\"outcome\":\"error\""));
    assert!(!log_text.contains("should-fail"));
}

#[tokio::test]
async fn mock_keychain_unavailable_reports_false_and_errors_on_access() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().expect("temp dir");
    let dir_str = dir.path().to_str().expect("utf8 path");
    let _env = EnvGuard::set(&[
        ("LIBRAGENT_E2E_CONTROL", Some("1")),
        ("LIBRAGENT_E2E_MOCK_KEYCHAIN", Some("unavailable")),
        ("LIBRAGENT_DATA_DIR", Some(dir_str)),
        ("LIBRAGENT_E2E_MOCK_KEYCHAIN_LOG", None),
    ]);

    let store = KeychainSecretStore;
    assert!(!store.available());
    assert!(store.set("srv-1", "API_KEY", "v").await.is_err());
    assert!(store.get("srv-1", "API_KEY").await.is_err());
}

#[test]
fn mock_keychain_module_stays_compile_time_gated_to_debug_assertions() {
    let source = include_str!("../src/mcp/keychain.rs");
    assert!(
        source.contains("#[cfg(debug_assertions)]\nmod e2e_mock"),
        "e2e_mock must stay gated behind #[cfg(debug_assertions)]"
    );
}

/// Gatekeeper G1.1: the mock shares the control plane's guard, so without an
/// explicit `LIBRAGENT_DATA_DIR` it stays inactive — no temp-dir store, no
/// temp-dir log, real keychain semantics apply.
#[tokio::test]
async fn mock_keychain_inactive_without_explicit_data_dir() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _env = EnvGuard::set(&[
        ("LIBRAGENT_E2E_CONTROL", Some("1")),
        ("LIBRAGENT_E2E_MOCK_KEYCHAIN", Some("succeed")),
        ("LIBRAGENT_DATA_DIR", None),
        ("LIBRAGENT_E2E_MOCK_KEYCHAIN_LOG", None),
    ]);

    // The removed fallback location. If anything recreates it, that is the
    // regression this test exists to catch.
    let sentinel_dir = std::env::temp_dir().join("libragent-e2e-mock-keychain");
    let sentinel_log = std::env::temp_dir().join("libragent-e2e-mock-keychain.log");
    let _ = std::fs::remove_dir_all(&sentinel_dir);

    let store = KeychainSecretStore;
    // An active mock logs here on the first available() call; an inactive one
    // must produce nothing at any predictable path.
    let _ = store.available();

    assert!(
        !sentinel_dir.exists(),
        "mock keychain must not create a temp-dir store without LIBRAGENT_DATA_DIR"
    );
    assert!(
        !sentinel_log.exists(),
        "mock keychain must not log to the temp dir without LIBRAGENT_DATA_DIR"
    );
    let _ = std::fs::remove_dir_all(&sentinel_dir);
}
