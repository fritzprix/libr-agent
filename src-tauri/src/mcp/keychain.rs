/// Secure token storage using OS keychain
///
/// This module provides cross-platform secure storage for OAuth tokens using the system keychain:
/// - macOS: Keychain Access
/// - Windows: Credential Manager
/// - Linux: Secret Service API / libsecret
use keyring::Entry;

const SERVICE_NAME: &str = "com.libr-agent.mcp";

/// Stores an OAuth token securely in the OS keychain
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server
/// * `token` - The OAuth access token to store
///
/// # Returns
/// A `Result` indicating success or an error string
pub async fn store_token_securely(server_id: &str, token: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, server_id)
        .map_err(|e| format!("Failed to create keychain entry: {e}"))?;

    entry
        .set_password(token)
        .map_err(|e| format!("Failed to store token in keychain: {e}"))?;

    log::info!("Stored OAuth token securely for server: {server_id}");
    Ok(())
}

/// Retrieves an OAuth token from the OS keychain
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server
///
/// # Returns
/// A `Result` containing `Some(token)` if found, `None` if not found, or an error
pub async fn get_cached_token(server_id: &str) -> Result<Option<String>, String> {
    let entry = Entry::new(SERVICE_NAME, server_id)
        .map_err(|e| format!("Failed to create keychain entry: {e}"))?;

    match entry.get_password() {
        Ok(token) => {
            log::debug!("Retrieved cached token for server: {server_id}");
            Ok(Some(token))
        }
        Err(keyring::Error::NoEntry) => {
            log::debug!("No cached token found for server: {server_id}");
            Ok(None)
        }
        Err(e) => Err(format!("Failed to retrieve token from keychain: {e}")),
    }
}

/// Deletes an OAuth token from the OS keychain
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server
///
/// # Returns
/// A `Result` indicating success or an error string
pub async fn delete_token(server_id: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, server_id)
        .map_err(|e| format!("Failed to create keychain entry: {e}"))?;

    entry
        .delete_credential()
        .map_err(|e| format!("Failed to delete token from keychain: {e}"))?;

    log::info!("Deleted OAuth token for server: {server_id}");
    Ok(())
}

/// MCP server secret values (`{{secret:NAME}}` templates) in the OS keychain,
/// under the account `<server_id>:<NAME>`.
pub struct KeychainSecretStore;

impl KeychainSecretStore {
    fn entry(server_id: &str, name: &str) -> Result<Entry, String> {
        Entry::new(SERVICE_NAME, &format!("{server_id}:{name}"))
            .map_err(|e| format!("Failed to create keychain entry: {e}"))
    }
}

/// Whether the OS keychain works on this machine. Checked once per process: the
/// credential store must start, and a read must not fail with a platform or access
/// error (for example Linux with no Secret Service running, or WSL).
fn keychain_available() -> bool {
    #[cfg(debug_assertions)]
    if let Some(available) = e2e_mock::mock_available() {
        return available;
    }

    static AVAILABLE: once_cell::sync::Lazy<bool> = once_cell::sync::Lazy::new(|| {
        if let Err(e) = Entry::store_status() {
            log::warn!("OS keychain is not available: {e}");
            return false;
        }
        let probe =
            Entry::new(SERVICE_NAME, "availability-probe").map(|entry| entry.get_password());
        match probe {
            Ok(Ok(_)) | Ok(Err(keyring::Error::NoEntry)) => true,
            Ok(Err(
                e @ (keyring::Error::PlatformFailure(_) | keyring::Error::NoStorageAccess(_)),
            ))
            | Err(e) => {
                log::warn!("OS keychain is not available: {e}");
                false
            }
            Ok(Err(_)) => true,
        }
    });
    *AVAILABLE
}

#[async_trait::async_trait]
impl crate::mcp::secret_templates::SecretStore for KeychainSecretStore {
    async fn get(&self, server_id: &str, name: &str) -> Result<Option<String>, String> {
        #[cfg(debug_assertions)]
        if let Some(result) = e2e_mock::mock_get(server_id, name) {
            return result;
        }

        match Self::entry(server_id, name)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("Failed to read secret '{name}' from keychain: {e}")),
        }
    }

    async fn set(
        &self,
        server_id: &str,
        name: &str,
        value: &str,
    ) -> Result<Option<String>, String> {
        #[cfg(debug_assertions)]
        if let Some(result) = e2e_mock::mock_set(server_id, name, value) {
            return result;
        }

        Self::entry(server_id, name)?
            .set_password(value)
            .map(|()| None)
            .map_err(|e| format!("Failed to store secret '{name}' in keychain: {e}"))
    }

    async fn delete(&self, server_id: &str, name: &str) -> Result<(), String> {
        #[cfg(debug_assertions)]
        if let Some(result) = e2e_mock::mock_delete(server_id, name) {
            return result;
        }

        match Self::entry(server_id, name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!(
                "Failed to delete secret '{name}' from keychain: {e}"
            )),
        }
    }

    fn available(&self) -> bool {
        keychain_available()
    }
}

/// Debug-only mock keychain for unattended E2E tests (#2904).
///
/// Active only when the E2E control plane itself is enabled — debug build +
/// `LIBRAGENT_E2E_CONTROL=1` + an **explicit `LIBRAGENT_DATA_DIR`** (the same
/// `e2e_control_enabled()` guard) — AND `LIBRAGENT_E2E_MOCK_KEYCHAIN`
/// (`succeed` | `fail-write` | `fail-read` | `unavailable`) is set. Without an
/// explicit data dir the mock is inactive and the real keychain is used, so a
/// developer's debug build never writes plaintext secrets to a predictable
/// path. Never compiled into release builds and never logs secret values.
#[cfg(debug_assertions)]
mod e2e_mock {
    use std::collections::BTreeMap;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicI8, Ordering};
    use std::sync::Mutex;

    static FILE_LOCK: once_cell::sync::Lazy<Mutex<()>> =
        once_cell::sync::Lazy::new(|| Mutex::new(()));

    /// Mock mode whose `available` outcome has already been logged (-1 = none).
    static AVAILABLE_LOGGED_MODE: AtomicI8 = AtomicI8::new(-1);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MockMode {
        Succeed,
        FailWrite,
        FailRead,
        Unavailable,
    }

    impl MockMode {
        fn as_str(self) -> &'static str {
            match self {
                Self::Succeed => "succeed",
                Self::FailWrite => "fail-write",
                Self::FailRead => "fail-read",
                Self::Unavailable => "unavailable",
            }
        }
    }

    fn active_mode() -> Option<MockMode> {
        // Same guard as the control plane (token.rs): debug build, opt-in env,
        // and an explicit disposable data dir. The mock must never act on a
        // primary profile just because two env vars leaked into a shell.
        if !crate::server::e2e_control::e2e_control_enabled() {
            return None;
        }
        match std::env::var("LIBRAGENT_E2E_MOCK_KEYCHAIN")
            .ok()
            .as_deref()
            .map(str::trim)
        {
            Some("succeed") => Some(MockMode::Succeed),
            Some("fail-write") => Some(MockMode::FailWrite),
            Some("fail-read") => Some(MockMode::FailRead),
            Some("unavailable") => Some(MockMode::Unavailable),
            _ => None,
        }
    }

    fn base_dir() -> Option<PathBuf> {
        // No temp-dir fallback: an explicit LIBRAGENT_DATA_DIR is the only
        // place plaintext mock secrets may land.
        let dir = std::env::var("LIBRAGENT_DATA_DIR").ok()?;
        let trimmed = dir.trim();
        (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
    }

    fn store_path() -> Option<PathBuf> {
        Some(base_dir()?.join("e2e_mock_keychain.json"))
    }

    fn log_path() -> Option<PathBuf> {
        if let Ok(path) = std::env::var("LIBRAGENT_E2E_MOCK_KEYCHAIN_LOG") {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return Some(PathBuf::from(trimmed));
            }
        }
        Some(base_dir()?.join("e2e_mock_keychain.log"))
    }

    fn append_log(op: &str, mode: MockMode, server_id: &str, name: &str, outcome: &str) {
        let Some(path) = log_path() else { return };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let line = serde_json::json!({
                "op": op,
                "mode": mode.as_str(),
                "serverId": server_id,
                "name": name,
                "outcome": outcome,
            });
            let _ = writeln!(file, "{line}");
        }
    }

    fn read_map() -> BTreeMap<String, String> {
        let Some(path) = store_path() else {
            return BTreeMap::new();
        };
        fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    /// Owner-only (0600 on unix) store file — the mock holds plaintext secrets.
    fn write_map(map: &BTreeMap<String, String>) -> Result<(), String> {
        let path = store_path()
            .ok_or_else(|| "Mock keychain requires an explicit LIBRAGENT_DATA_DIR".to_string())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create mock keychain dir: {e}"))?;
        }
        let body = serde_json::to_string_pretty(map)
            .map_err(|e| format!("Failed to serialize mock keychain: {e}"))?;
        let mut options = OpenOptions::new();
        options.create(true).write(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&path)
            .map_err(|e| format!("Failed to write mock keychain: {e}"))?;
        file.write_all(body.as_bytes())
            .map_err(|e| format!("Failed to write mock keychain: {e}"))
    }

    pub(super) fn mock_available() -> Option<bool> {
        let mode = active_mode()?;
        let available = mode != MockMode::Unavailable;
        // Log once per mode per process: keychain_available() is queried on
        // every secret operation, and a per-call line buries the real ops.
        let mode_code: i8 = match mode {
            MockMode::Succeed => 0,
            MockMode::FailWrite => 1,
            MockMode::FailRead => 2,
            MockMode::Unavailable => 3,
        };
        if AVAILABLE_LOGGED_MODE.swap(mode_code, Ordering::SeqCst) != mode_code {
            append_log(
                "available",
                mode,
                "",
                "",
                if available { "true" } else { "false" },
            );
        }
        Some(available)
    }

    /// Store key. server_id is a slug and name an env-var-style token, so the
    /// ASCII unit separator (\u{1f}) cannot appear in either — a plain `:`
    /// could collide across `"{a}:{b}"` splits (gatekeeper S1.4).
    fn store_key(server_id: &str, name: &str) -> String {
        format!("{server_id}\u{1f}{name}")
    }

    pub(super) fn mock_get(server_id: &str, name: &str) -> Option<Result<Option<String>, String>> {
        let mode = active_mode()?;
        let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(mode, MockMode::FailRead | MockMode::Unavailable) {
            append_log("get", mode, server_id, name, "error");
            return Some(Err(format!(
                "Mock keychain read failed for '{name}' (LIBRAGENT_E2E_MOCK_KEYCHAIN={})",
                mode.as_str()
            )));
        }
        let key = store_key(server_id, name);
        let val = read_map().get(&key).cloned();
        append_log(
            "get",
            mode,
            server_id,
            name,
            if val.is_some() { "hit" } else { "miss" },
        );
        Some(Ok(val))
    }

    pub(super) fn mock_set(
        server_id: &str,
        name: &str,
        value: &str,
    ) -> Option<Result<Option<String>, String>> {
        let mode = active_mode()?;
        let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(mode, MockMode::FailWrite | MockMode::Unavailable) {
            append_log("set", mode, server_id, name, "error");
            return Some(Err(format!(
                "Mock keychain write failed for '{name}' (LIBRAGENT_E2E_MOCK_KEYCHAIN={})",
                mode.as_str()
            )));
        }
        let mut map = read_map();
        map.insert(store_key(server_id, name), value.to_string());
        let res = write_map(&map).map(|()| None);
        append_log(
            "set",
            mode,
            server_id,
            name,
            if res.is_ok() { "ok" } else { "error" },
        );
        Some(res)
    }

    pub(super) fn mock_delete(server_id: &str, name: &str) -> Option<Result<(), String>> {
        let mode = active_mode()?;
        let _guard = FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if mode == MockMode::Unavailable {
            append_log("delete", mode, server_id, name, "error");
            return Some(Err(format!(
                "Mock keychain delete failed for '{name}' (LIBRAGENT_E2E_MOCK_KEYCHAIN=unavailable)"
            )));
        }
        let mut map = read_map();
        map.remove(&store_key(server_id, name));
        let res = write_map(&map);
        append_log(
            "delete",
            mode,
            server_id,
            name,
            if res.is_ok() { "ok" } else { "error" },
        );
        Some(res)
    }
}

/// Checks if a token exists in the keychain without retrieving it
///
/// # Arguments
/// * `server_id` - The unique identifier for the MCP server
///
/// # Returns
/// `true` if a token exists, `false` otherwise
pub async fn has_token(server_id: &str) -> bool {
    matches!(get_cached_token(server_id).await, Ok(Some(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires OS keychain service (not available in CI/headless env)"]
    async fn test_store_and_retrieve_token() {
        let server_id = "test-server-keychain-store";
        let token = "test-token-12345";

        // Clean up any existing token
        let _ = delete_token(server_id).await;

        // Store token
        let result = store_token_securely(server_id, token).await;
        assert!(result.is_ok());

        // Retrieve token
        let retrieved = get_cached_token(server_id).await;
        assert!(retrieved.is_ok());
        assert_eq!(retrieved.unwrap(), Some(token.to_string()));

        // Clean up
        let _ = delete_token(server_id).await;
    }

    #[tokio::test]
    #[ignore = "Requires OS keychain service (not available in CI/headless env)"]
    async fn test_delete_token() {
        let server_id = "test-server-keychain-delete";
        let token = "test-token-67890";

        // Store token
        let _ = store_token_securely(server_id, token).await;

        // Delete token
        let result = delete_token(server_id).await;
        assert!(result.is_ok());

        // Verify deletion
        let retrieved = get_cached_token(server_id).await;
        assert!(retrieved.is_ok());
        assert_eq!(retrieved.unwrap(), None);
    }

    #[tokio::test]
    #[ignore = "Requires OS keychain service (not available in CI/headless env)"]
    async fn test_has_token() {
        let server_id = "test-server-keychain-has";
        let token = "test-token-abcde";

        // Clean up
        let _ = delete_token(server_id).await;

        // Should not have token
        assert!(!has_token(server_id).await);

        // Store token
        let _ = store_token_securely(server_id, token).await;

        // Should have token
        assert!(has_token(server_id).await);

        // Clean up
        let _ = delete_token(server_id).await;
    }
}
