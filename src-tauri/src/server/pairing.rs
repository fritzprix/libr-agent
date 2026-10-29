//! Session API key pairing (issue #2348):
//!
//! Provides out-of-band agent pairing with 1-time code approval.
//! External agents request pairing with a 6-character code (`xxx-yyy`) and agent name.
//! LibrAgent UI prompts the user to enter the code. Upon correct entry, an API key is
//! generated, saved in hashed form to `session_api_keys.json`, and returned to the waiting agent.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};
use warp::hyper::StatusCode;

use super::api_key::{generate_key, hash_key, hashes_equal, KeyHash};

const KEY_FILE_NAME: &str = "session_api_keys.json";
pub const PAIRING_TIMEOUT_SECS: u64 = 120;
pub const MAX_FAILED_ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssuedApiKey {
    pub id: String,
    pub name: String,
    /// Hex-encoded SHA-256 hash of the key
    pub key_hash: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<String>,
}

pub struct PendingPairing {
    pub id: String,
    pub name: String,
    pub code: String, // lowercase normalized
    pub expires_at: Instant,
    pub failed_attempts: u32,
    pub responder: tokio::sync::oneshot::Sender<Result<String, String>>,
}

static PENDING_PAIRING: Mutex<Option<PendingPairing>> = Mutex::new(None);
static KEYS_CACHE: RwLock<Option<Result<Vec<IssuedApiKey>, String>>> = RwLock::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingPairingInfo {
    pub id: String,
    pub name: String,
    pub expires_in_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedApiKeySummary {
    pub id: String,
    pub name: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PairingRequestBody {
    pub name: String,
    pub code: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingSuccessResponse {
    pub api_key: String,
    pub name: String,
    pub id: String,
}

pub fn hash_to_hex(hash: &KeyHash) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn hex_to_hash(hex: &str) -> Option<KeyHash> {
    if hex.len() != 64 {
        return None;
    }
    let mut hash = [0u8; 32];
    for (i, chunk) in hex.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let str_chunk = std::str::from_utf8(chunk).ok()?;
        hash[i] = u8::from_str_radix(str_chunk, 16).ok()?;
    }
    Some(hash)
}

/// Checks if code matches `xxx-yyy` (3 alphanumeric chars, hyphen, 3 alphanumeric chars).
pub fn is_valid_pairing_code_format(code: &str) -> bool {
    let trimmed = code.trim();
    if trimmed.len() != 7 {
        return false;
    }
    let bytes = trimmed.as_bytes();
    bytes[3] == b'-'
        && bytes[0..3].iter().all(|b| b.is_ascii_alphanumeric())
        && bytes[4..7].iter().all(|b| b.is_ascii_alphanumeric())
}

pub fn keys_file_path() -> PathBuf {
    let data_dir = crate::profile::data_dir(crate::profile::resolve_profile());
    data_dir.join(KEY_FILE_NAME)
}

pub fn invalidate_keys_cache() {
    if let Ok(mut guard) = KEYS_CACHE.write() {
        *guard = None;
    }
}

pub fn load_issued_keys() -> Result<Vec<IssuedApiKey>, String> {
    if let Ok(guard) = KEYS_CACHE.read() {
        if let Some(Ok(keys)) = guard.as_ref() {
            return Ok(keys.clone());
        }
    }

    let path = keys_file_path();
    let res = load_issued_keys_from_path(&path);

    if let Ok(mut guard) = KEYS_CACHE.write() {
        if let Ok(keys) = &res {
            *guard = Some(Ok(keys.clone()));
        } else {
            *guard = None;
        }
    }

    res
}

fn load_issued_keys_from_path(path: &std::path::Path) -> Result<Vec<IssuedApiKey>, String> {
    match fs::metadata(path) {
        Ok(_meta) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if _meta.permissions().mode() & 0o077 != 0 {
                    return Err(
                        "key file must not be accessible by group or others (use mode 0600)"
                            .to_string(),
                    );
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(e) => {
            return Err(format!("failed to check key file metadata: {e}"));
        }
    }

    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("failed to read key file: {e}")),
    };

    let keys: Vec<IssuedApiKey> =
        serde_json::from_str(&contents).map_err(|e| format!("failed to parse key file: {e}"))?;

    Ok(keys)
}

pub fn save_issued_keys_all(keys: &[IssuedApiKey]) -> Result<(), std::io::Error> {
    let path = keys_file_path();
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "invalid key file path")
    })?;
    fs::create_dir_all(parent)?;

    let json = serde_json::to_string_pretty(keys).map_err(std::io::Error::other)?;

    // Atomic write: write to temp file in same directory, sync to disk, then rename/persist
    let mut temp_file = tempfile::NamedTempFile::new_in(parent)?;

    use std::io::Write;
    temp_file.write_all(json.as_bytes())?;
    temp_file.flush()?;
    temp_file.as_file().sync_all()?;

    temp_file.persist(&path).map_err(|e| e.error)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }

    if let Ok(mut guard) = KEYS_CACHE.write() {
        *guard = Some(Ok(keys.to_vec()));
    }
    Ok(())
}

fn save_new_key(key: &IssuedApiKey) -> Result<(), std::io::Error> {
    let mut keys = load_issued_keys().map_err(std::io::Error::other)?;
    keys.push(key.clone());
    save_issued_keys_all(&keys)
}

/// Returns true if there is any active key in the persistent store.
/// Returns Err if the key store could not be read or parsed (fail-closed signal).
pub fn has_any_active_issued_key() -> Result<bool, String> {
    let keys = load_issued_keys()?;
    Ok(keys.iter().any(|key| key.revoked_at.is_none()))
}

/// Verifies whether the provided token matches any active issued API key.
pub fn is_token_accepted_by_issued_keys(token: &str) -> bool {
    let Ok(keys) = load_issued_keys() else {
        return false;
    };
    let token_hash = hash_key(token);
    keys.iter()
        .filter(|key| key.revoked_at.is_none())
        .any(|key| {
            if let Some(expected_hash) = hex_to_hash(&key.key_hash) {
                hashes_equal(&token_hash, &expected_hash)
            } else {
                false
            }
        })
}

/// HTTP handler implementation for `POST /api/pairings`.
pub async fn handle_pairing_request(
    body: PairingRequestBody,
) -> Result<PairingSuccessResponse, (StatusCode, String)> {
    // Fail closed if the key storage cannot be read/parsed
    if let Err(e) = load_issued_keys() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Session API key storage is in an error state: {e}"),
        ));
    }

    let name = body.name.trim();
    if name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Agent name is required".to_string(),
        ));
    }
    if name.len() > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Agent name must be 100 characters or fewer".to_string(),
        ));
    }
    if !is_valid_pairing_code_format(&body.code) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Code must be in xxx-yyy format with alphanumeric characters".to_string(),
        ));
    }

    let code_normalized = body.code.trim().to_ascii_lowercase();
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel();

    {
        let mut lock = PENDING_PAIRING.lock().map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to acquire pairing lock".to_string(),
            )
        })?;

        let now = Instant::now();
        if let Some(existing) = lock.as_ref() {
            if now < existing.expires_at {
                return Err((
                    StatusCode::CONFLICT,
                    "Another pairing request is already pending. Please complete or wait for it to expire."
                        .to_string(),
                ));
            }
        }

        *lock = Some(PendingPairing {
            id: id.clone(),
            name: name.to_string(),
            code: code_normalized,
            expires_at: now + Duration::from_secs(PAIRING_TIMEOUT_SECS),
            failed_attempts: 0,
            responder: tx,
        });
    }

    // Emit Tauri event to frontends
    if let Some(app_handle) = crate::state::get_app_handle() {
        use tauri::Emitter;
        let _ = app_handle.emit(
            "session_api_pairing_requested",
            PendingPairingInfo {
                id: id.clone(),
                name: name.to_string(),
                expires_in_seconds: PAIRING_TIMEOUT_SECS,
            },
        );
    }

    // Await approval or timeout
    let wait_result = tokio::time::timeout(Duration::from_secs(PAIRING_TIMEOUT_SECS), rx).await;

    match wait_result {
        Ok(Ok(Ok(api_key))) => Ok(PairingSuccessResponse {
            api_key,
            name: name.to_string(),
            id,
        }),
        Ok(Ok(Err(rejection))) => Err((StatusCode::FORBIDDEN, rejection)),
        Ok(Err(_closed)) => Err((
            StatusCode::FORBIDDEN,
            "Pairing request was cancelled".to_string(),
        )),
        Err(_timeout) => {
            if let Ok(mut lock) = PENDING_PAIRING.lock() {
                if let Some(p) = lock.as_ref() {
                    if p.id == id {
                        *lock = None;
                    }
                }
            }
            Err((
                StatusCode::REQUEST_TIMEOUT,
                "Pairing request timed out".to_string(),
            ))
        }
    }
}

pub fn get_pending_pairing() -> Option<PendingPairingInfo> {
    let mut lock = PENDING_PAIRING.lock().ok()?;
    let now = Instant::now();
    if let Some(pending) = lock.as_ref() {
        if now < pending.expires_at {
            let remaining_secs = pending.expires_at.duration_since(now).as_secs();
            return Some(PendingPairingInfo {
                id: pending.id.clone(),
                name: pending.name.clone(),
                expires_in_seconds: remaining_secs,
            });
        }
    }
    *lock = None;
    None
}

pub fn approve_pairing(code: &str) -> Result<IssuedApiKeySummary, String> {
    let mut lock = PENDING_PAIRING
        .lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let pending_ref = lock
        .as_mut()
        .ok_or_else(|| "No pending pairing request found".to_string())?;

    if Instant::now() >= pending_ref.expires_at {
        let _ = lock.take();
        return Err("Pairing request has expired".to_string());
    }

    let input_code = code.trim().to_ascii_lowercase();
    if input_code != pending_ref.code {
        pending_ref.failed_attempts += 1;
        if pending_ref.failed_attempts >= MAX_FAILED_ATTEMPTS {
            if let Some(p) = lock.take() {
                let _ = p.responder.send(Err(
                    "Pairing rejected: 3 incorrect code attempts".to_string()
                ));
            }
            return Err("Code incorrect 3 times. Pairing request cancelled.".to_string());
        }
        let remaining = MAX_FAILED_ATTEMPTS - pending_ref.failed_attempts;
        return Err(format!("Incorrect code. Attempts remaining: {remaining}"));
    }

    if pending_ref.responder.is_closed() {
        let _ = lock.take();
        return Err("Pairing request connection was closed by the client".to_string());
    }

    let raw_key = generate_key().map_err(|e| format!("Failed to generate API key: {e}"))?;
    let hash = hash_key(&raw_key);
    let hex_hash = hash_to_hex(&hash);
    let now_iso = chrono::Utc::now().to_rfc3339();

    let issued = IssuedApiKey {
        id: pending_ref.id.clone(),
        name: pending_ref.name.clone(),
        key_hash: hex_hash,
        created_at: now_iso.clone(),
        revoked_at: None,
    };

    save_new_key(&issued).map_err(|e| format!("Failed to persist issued key: {e}"))?;

    let pending = lock.take().expect("pending exists");
    let summary = IssuedApiKeySummary {
        id: pending.id,
        name: pending.name,
        created_at: now_iso,
        revoked_at: None,
    };

    let _ = pending.responder.send(Ok(raw_key));

    Ok(summary)
}

pub fn reject_pending_pairing() -> Result<(), String> {
    let mut lock = PENDING_PAIRING
        .lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let pending = lock
        .take()
        .ok_or_else(|| "No pending pairing request found".to_string())?;
    let _ = pending
        .responder
        .send(Err("Pairing request was rejected by the user".to_string()));
    Ok(())
}

pub fn list_issued_keys() -> Result<Vec<IssuedApiKeySummary>, String> {
    invalidate_keys_cache();
    let keys = load_issued_keys()?;
    Ok(keys
        .into_iter()
        .filter(|k| k.revoked_at.is_none())
        .map(|k| IssuedApiKeySummary {
            id: k.id,
            name: k.name,
            created_at: k.created_at,
            revoked_at: None,
        })
        .collect())
}

/// Revoking a key deletes it from the persistent key file (does not keep a revoked_at tombstone).
pub fn revoke_issued_key(id: &str) -> Result<(), String> {
    let mut keys =
        load_issued_keys().map_err(|e| format!("Cannot delete key: key storage error: {e}"))?;
    let original_len = keys.len();
    keys.retain(|k| k.id != id);

    if keys.len() == original_len {
        return Err("API key not found".to_string());
    }

    save_issued_keys_all(&keys).map_err(|e| format!("Failed to update keys: {e}"))?;
    Ok(())
}

#[cfg(test)]
pub struct TestEnvDataDir {
    _lock: std::sync::MutexGuard<'static, ()>,
    prev_env: Option<String>,
}

#[cfg(test)]
static TEST_SERIAL_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub fn test_env_data_dir(path: &std::path::Path) -> TestEnvDataDir {
    let lock = TEST_SERIAL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let prev_env = std::env::var("LIBRAGENT_DATA_DIR").ok();
    std::env::set_var("LIBRAGENT_DATA_DIR", path);
    *PENDING_PAIRING.lock().unwrap_or_else(|p| p.into_inner()) = None;
    invalidate_keys_cache();
    TestEnvDataDir {
        _lock: lock,
        prev_env,
    }
}

#[cfg(test)]
impl Drop for TestEnvDataDir {
    fn drop(&mut self) {
        if let Some(prev) = &self.prev_env {
            std::env::set_var("LIBRAGENT_DATA_DIR", prev);
        } else {
            std::env::remove_var("LIBRAGENT_DATA_DIR");
        }
        *PENDING_PAIRING.lock().unwrap_or_else(|p| p.into_inner()) = None;
        invalidate_keys_cache();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_pairing_code_format() {
        assert!(is_valid_pairing_code_format("abc-123"));
        assert!(is_valid_pairing_code_format("ABC-XYZ"));
        assert!(is_valid_pairing_code_format("999-000"));
        assert!(is_valid_pairing_code_format(" a1b-c2d "));

        assert!(!is_valid_pairing_code_format(""));
        assert!(!is_valid_pairing_code_format("abc1234"));
        assert!(!is_valid_pairing_code_format("abcd-123"));
        assert!(!is_valid_pairing_code_format("abc-1234"));
        assert!(!is_valid_pairing_code_format("ab@-123"));
        assert!(!is_valid_pairing_code_format("abc_123"));
    }

    #[test]
    fn hex_hash_round_trip() {
        let hash = [42u8; 32];
        let hex = hash_to_hex(&hash);
        assert_eq!(hex.len(), 64);
        let decoded = hex_to_hash(&hex).expect("valid hex");
        assert_eq!(hash, decoded);
    }

    #[tokio::test]
    async fn wrong_code_three_times_cancels_request_and_issues_no_key() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = test_env_data_dir(temp.path());

        let req_task = tokio::spawn(handle_pairing_request(PairingRequestBody {
            name: "test-agent".to_string(),
            code: "abc-123".to_string(),
        }));

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(get_pending_pairing().is_some());

        // Attempt 1: wrong
        let err1 = approve_pairing("wrong1").unwrap_err();
        assert!(err1.contains("Attempts remaining: 2"), "{err1}");

        // Attempt 2: wrong
        let err2 = approve_pairing("wrong2").unwrap_err();
        assert!(err2.contains("Attempts remaining: 1"), "{err2}");

        // Attempt 3: wrong -> cancelled
        let err3 = approve_pairing("wrong3").unwrap_err();
        assert!(err3.contains("cancelled"), "{err3}");

        // Request task gets forbidden
        let res = req_task.await.unwrap();
        assert!(res.is_err());
        let (status, msg) = res.unwrap_err();
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(msg.contains("3 incorrect code attempts"));

        // No pending pairing, no keys issued
        assert!(get_pending_pairing().is_none());
        assert!(load_issued_keys().unwrap().is_empty());
    }

    #[tokio::test]
    async fn second_request_while_pending_returns_conflict_409() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = test_env_data_dir(temp.path());

        let req_task = tokio::spawn(handle_pairing_request(PairingRequestBody {
            name: "agent-1".to_string(),
            code: "aaa-111".to_string(),
        }));

        tokio::time::sleep(Duration::from_millis(50)).await;

        let conflict = handle_pairing_request(PairingRequestBody {
            name: "agent-2".to_string(),
            code: "bbb-222".to_string(),
        })
        .await;

        assert!(conflict.is_err());
        let (status, msg) = conflict.unwrap_err();
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(msg.contains("Another pairing request is already pending"));

        // Clean up first task by rejecting it
        reject_pending_pairing().unwrap();
        let _ = req_task.await;
    }

    #[tokio::test]
    async fn expired_request_cannot_be_approved() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = test_env_data_dir(temp.path());

        let (tx, _rx) = tokio::sync::oneshot::channel();
        {
            let mut lock = PENDING_PAIRING.lock().unwrap();
            *lock = Some(PendingPairing {
                id: "test-id".to_string(),
                name: "agent".to_string(),
                code: "exp-123".to_string(),
                expires_at: Instant::now() - Duration::from_secs(1),
                failed_attempts: 0,
                responder: tx,
            });
        }

        let err = approve_pairing("exp-123").unwrap_err();
        assert!(err.contains("expired"), "{err}");
        assert!(get_pending_pairing().is_none());
    }

    #[tokio::test]
    async fn revoked_key_is_rejected_by_is_token_accepted_by_issued_keys() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = test_env_data_dir(temp.path());

        let req_task = tokio::spawn(handle_pairing_request(PairingRequestBody {
            name: "agent-d".to_string(),
            code: "ddd-444".to_string(),
        }));

        tokio::time::sleep(Duration::from_millis(50)).await;
        let summary = approve_pairing("ddd-444").unwrap();
        let resp = req_task.await.unwrap().unwrap();

        assert_eq!(summary.id, resp.id);
        assert!(is_token_accepted_by_issued_keys(&resp.api_key));

        // Revoke the key
        revoke_issued_key(&resp.id).unwrap();

        // Must now be rejected
        assert!(!is_token_accepted_by_issued_keys(&resp.api_key));
    }

    #[test]
    fn file_not_found_treated_as_zero_keys() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = test_env_data_dir(temp.path());

        let keys = load_issued_keys().expect("missing file should yield Ok([])");
        assert!(keys.is_empty());

        let has_keys = has_any_active_issued_key().expect("should return Ok(false)");
        assert!(!has_keys);

        let list = list_issued_keys().expect("should return Ok([])");
        assert!(list.is_empty());
    }

    #[test]
    fn corrupted_key_file_fails_closed_and_prevents_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = test_env_data_dir(temp.path());

        let file_path = keys_file_path();
        let bad_content = "{corrupted json content";
        fs::write(&file_path, bad_content).unwrap();

        // 1. Reading fails closed with Err
        assert!(load_issued_keys().is_err());
        assert!(has_any_active_issued_key().is_err());
        assert!(list_issued_keys().is_err());

        // 2. save_new_key refuses to overwrite
        let sample = IssuedApiKey {
            id: "key-sample".to_string(),
            name: "sample".to_string(),
            key_hash: "a".repeat(64),
            created_at: chrono::Utc::now().to_rfc3339(),
            revoked_at: None,
        };
        assert!(save_new_key(&sample).is_err());
        let on_disk = fs::read_to_string(&file_path).unwrap();
        assert_eq!(on_disk, bad_content, "file must not be overwritten");

        // 3. revoke_issued_key refuses to overwrite
        assert!(revoke_issued_key("key-sample").is_err());
        let on_disk_after_revoke = fs::read_to_string(&file_path).unwrap();
        assert_eq!(
            on_disk_after_revoke, bad_content,
            "file must not be overwritten on revoke"
        );
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_key_file_fails_closed_and_prevents_overwrite() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let _guard = test_env_data_dir(temp.path());

        let file_path = keys_file_path();
        let sample = IssuedApiKey {
            id: "key-1".to_string(),
            name: "test".to_string(),
            key_hash: "b".repeat(64),
            created_at: chrono::Utc::now().to_rfc3339(),
            revoked_at: None,
        };
        save_issued_keys_all(&[sample.clone()]).unwrap();

        // Case A: Mode 0644 (accessible to others -> rejected under 0600 policy)
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o644)).unwrap();
        invalidate_keys_cache();

        assert!(load_issued_keys().is_err());
        assert!(has_any_active_issued_key().is_err());
        assert!(save_new_key(&sample).is_err());
        assert!(revoke_issued_key("key-1").is_err());

        // Case B: Mode 0000 (completely unreadable)
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o000)).unwrap();
        invalidate_keys_cache();

        assert!(load_issued_keys().is_err());
        assert!(has_any_active_issued_key().is_err());
        assert!(save_new_key(&sample).is_err());
        assert!(revoke_issued_key("key-1").is_err());

        // Restore permission for clean tempdir cleanup
        let _ = fs::set_permissions(&file_path, fs::Permissions::from_mode(0o600));
    }

    #[test]
    fn key_revocation_removes_entry_from_file() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = test_env_data_dir(temp.path());

        let file_path = keys_file_path();
        let key1 = IssuedApiKey {
            id: "key-1".to_string(),
            name: "agent-1".to_string(),
            key_hash: "1".repeat(64),
            created_at: chrono::Utc::now().to_rfc3339(),
            revoked_at: None,
        };
        let key2 = IssuedApiKey {
            id: "key-2".to_string(),
            name: "agent-2".to_string(),
            key_hash: "2".repeat(64),
            created_at: chrono::Utc::now().to_rfc3339(),
            revoked_at: None,
        };

        save_issued_keys_all(&[key1.clone(), key2.clone()]).unwrap();
        let disk_initial = fs::read_to_string(&file_path).unwrap();
        assert!(disk_initial.contains("key-1"));
        assert!(disk_initial.contains("key-2"));

        // Revoke/delete key-1
        revoke_issued_key("key-1").unwrap();

        // Raw file check: key-1 must NOT remain in the file
        let disk_after_k1 = fs::read_to_string(&file_path).unwrap();
        assert!(!disk_after_k1.contains("key-1"));
        assert!(disk_after_k1.contains("key-2"));

        let list = list_issued_keys().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "key-2");

        // Revoke/delete key-2
        revoke_issued_key("key-2").unwrap();

        let disk_after_k2 = fs::read_to_string(&file_path).unwrap();
        assert!(!disk_after_k2.contains("key-2"));
        assert_eq!(list_issued_keys().unwrap().len(), 0);

        // Deleting already deleted key returns error
        assert!(revoke_issued_key("key-1").is_err());
    }

    #[test]
    fn atomic_write_creates_valid_file_with_0600_permissions() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = test_env_data_dir(temp.path());

        let file_path = keys_file_path();
        let sample = IssuedApiKey {
            id: "key-atomic".to_string(),
            name: "atomic-agent".to_string(),
            key_hash: "c".repeat(64),
            created_at: chrono::Utc::now().to_rfc3339(),
            revoked_at: None,
        };

        save_issued_keys_all(&[sample.clone()]).unwrap();
        assert!(file_path.exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&file_path).unwrap().permissions().mode();
            assert_eq!(
                mode & 0o777,
                0o600,
                "file must have 0600 mode on unix (got {mode:o})"
            );
        }

        let loaded = load_issued_keys().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "key-atomic");
    }

    #[tokio::test]
    async fn persisted_file_contains_only_hash_never_raw_key() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = test_env_data_dir(temp.path());

        let req_task = tokio::spawn(handle_pairing_request(PairingRequestBody {
            name: "agent-e".to_string(),
            code: "eee-555".to_string(),
        }));

        tokio::time::sleep(Duration::from_millis(50)).await;
        let summary = approve_pairing("eee-555").unwrap();
        let resp = req_task.await.unwrap().unwrap();

        let path = keys_file_path();
        let contents = std::fs::read_to_string(&path).unwrap();

        // Raw key must NEVER appear in the persisted JSON file
        assert!(!contents.contains(&resp.api_key));

        // Hex hash of the key must appear
        let expected_hex = hash_to_hex(&hash_key(&resp.api_key));
        assert!(contents.contains(&expected_hex));
        assert!(contents.contains(&summary.id));
    }
}
