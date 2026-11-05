//! Keychain-unavailable fallback for `{{secret:NAME}}` values (#2570).
//!
//! When the OS keychain does not work, the `{NAME: value}` map of a server is encrypted
//! with AES-256-GCM and stored in the `encrypted_secrets` column of its `mcp_servers`
//! row. The key is a random 256-bit app key in a file next to the database, readable by
//! the current user only. The key file is never exported.

use crate::mcp::secret_templates::SecretStore;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use aes_gcm::aead::{rand_core::RngCore, Aead, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) const KEY_FILE_NAME: &str = "mcp-secrets.key";
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;

/// Shown once, when the key file is created.
pub const FALLBACK_WARNING: &str = "Your OS keychain is not available. MCP secret values are encrypted with a key file on this computer. This protects them if only the database leaks, not if someone has the whole disk.";

/// The app key file in the app local data dir (next to the database).
pub fn app_key_path() -> PathBuf {
    crate::profile::data_dir(crate::profile::resolve_profile()).join(KEY_FILE_NAME)
}

/// Keychain entries deleted while the keychain was unavailable, next to the key file.
pub(crate) const PENDING_DELETES_FILE_NAME: &str = "mcp-secrets-pending-deletes.json";

/// `(server_id, name)` pairs whose keychain entry must still be removed.
type PendingDeletes = BTreeSet<(String, String)>;

/// Uses `primary` (the OS keychain) when it is available, and `fallback` otherwise.
///
/// The fallback only holds changes made while the keychain was unavailable, so it is
/// newer than the keychain by construction (#2588):
/// - a fallback value wins over the keychain copy and is moved to the keychain;
/// - a delete made during the outage is kept in a pending-deletes file, outside the
///   server row, and applied to the keychain before the next operation that finds the
///   keychain working. It does not need a read of the deleted name, and it survives the
///   deletion of the row.
pub struct LayeredSecretStore<'a> {
    primary: &'a dyn SecretStore,
    fallback: FileSecretStore<'a>,
    pending_deletes_path: PathBuf,
    /// Serializes the read-modify-write of the pending-deletes file.
    pending_lock: tokio::sync::Mutex<()>,
}

impl<'a> LayeredSecretStore<'a> {
    pub fn new(primary: &'a dyn SecretStore, fallback: FileSecretStore<'a>) -> Self {
        let pending_deletes_path = fallback.key_path.with_file_name(PENDING_DELETES_FILE_NAME);
        Self {
            primary,
            fallback,
            pending_deletes_path,
            pending_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Removes the keychain entries deleted during an outage. Entries that fail stay
    /// pending for the next try.
    async fn apply_pending_deletes(&self) {
        let _guard = self.pending_lock.lock().await;
        let pending = match read_pending_deletes(&self.pending_deletes_path) {
            Ok(pending) => pending,
            Err(e) => {
                log::warn!("{e}");
                return;
            }
        };
        if pending.is_empty() {
            return;
        }
        let mut remaining = PendingDeletes::new();
        for (server_id, name) in pending {
            if let Err(e) = self.primary.delete(&server_id, &name).await {
                log::warn!("Could not remove the keychain entry of secret '{name}' for MCP server '{server_id}' that was deleted during a keychain outage: {e}");
                remaining.insert((server_id, name));
            }
        }
        if let Err(e) = write_pending_deletes(&self.pending_deletes_path, &remaining) {
            log::warn!("{e}");
        }
    }

    async fn add_pending_delete(&self, server_id: &str, name: &str) -> Result<(), String> {
        let _guard = self.pending_lock.lock().await;
        let mut pending = read_pending_deletes(&self.pending_deletes_path)?;
        if pending.insert((server_id.to_string(), name.to_string())) {
            write_pending_deletes(&self.pending_deletes_path, &pending)?;
        }
        Ok(())
    }

    /// After the keychain holds the newest value of `name`, a fallback copy would win
    /// over it on a later read, and a pending delete that failed earlier would remove
    /// it. Failing to clear either must not undo the keychain save.
    async fn keychain_holds_newest(&self, server_id: &str, name: &str) {
        if let Err(e) = self.fallback.delete(server_id, name).await {
            log::warn!("Could not remove the fallback copy of secret '{name}' for MCP server '{server_id}': {e}");
        }
        let _guard = self.pending_lock.lock().await;
        let removed = read_pending_deletes(&self.pending_deletes_path).and_then(|mut pending| {
            if pending.remove(&(server_id.to_string(), name.to_string())) {
                write_pending_deletes(&self.pending_deletes_path, &pending)?;
            }
            Ok(())
        });
        if let Err(e) = removed {
            log::warn!("{e}");
        }
    }
}

#[async_trait::async_trait]
impl SecretStore for LayeredSecretStore<'_> {
    async fn get(&self, server_id: &str, name: &str) -> Result<Option<String>, String> {
        if !self.primary.available() {
            return self.fallback.get(server_id, name).await;
        }
        self.apply_pending_deletes().await;
        let fallback_value = match self.fallback.get(server_id, name).await {
            Ok(value) => value,
            // The fallback values are lost (for example with the key file); the keychain
            // copy is the newest value that can still be read.
            Err(e) => {
                log::warn!(
                    "Using the keychain copy of secret '{name}' for MCP server '{server_id}': {e}"
                );
                None
            }
        };
        let Some(value) = fallback_value else {
            return self.primary.get(server_id, name).await;
        };
        // The value is in hand; a keychain that rejects the write (for example locked)
        // keeps the fallback copy for a later try instead of failing the read.
        match self.primary.set(server_id, name, &value).await {
            Ok(_) => self.keychain_holds_newest(server_id, name).await,
            Err(e) => log::warn!("Keeping the fallback copy of secret '{name}' for MCP server '{server_id}': the keychain save failed: {e}"),
        }
        Ok(Some(value))
    }

    async fn set(
        &self,
        server_id: &str,
        name: &str,
        value: &str,
    ) -> Result<Option<String>, String> {
        if self.primary.available() {
            self.apply_pending_deletes().await;
            let notice = self.primary.set(server_id, name, value).await?;
            self.keychain_holds_newest(server_id, name).await;
            Ok(notice)
        } else {
            self.fallback.set(server_id, name, value).await
        }
    }

    async fn delete(&self, server_id: &str, name: &str) -> Result<(), String> {
        if self.primary.available() {
            self.apply_pending_deletes().await;
            self.primary.delete(server_id, name).await?;
            return self.fallback.delete(server_id, name).await;
        }
        // The caller drops the name either way, so the fallback copy is removed even
        // when the keychain delete cannot be recorded.
        let recorded = self.add_pending_delete(server_id, name).await;
        self.fallback.delete(server_id, name).await?;
        recorded
    }
}

fn read_pending_deletes(path: &Path) -> Result<PendingDeletes, String> {
    let unreadable = |e: &dyn std::fmt::Display| {
        format!(
            "Failed to read the pending MCP secret deletes {}: {e}",
            path.display()
        )
    };
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| unreadable(&e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PendingDeletes::new()),
        Err(e) => Err(unreadable(&e)),
    }
}

/// Replaces the file through a temporary file, so a reader never sees a partial list.
/// An empty list removes the file.
fn write_pending_deletes(path: &Path, pending: &PendingDeletes) -> Result<(), String> {
    let failed = |e: &dyn std::fmt::Display| {
        format!(
            "Failed to save the pending MCP secret deletes {}: {e}",
            path.display()
        )
    };
    if pending.is_empty() {
        return match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(failed(&e)),
        };
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| failed(&e))?;
    }
    let json = serde_json::to_vec(pending).map_err(|e| failed(&e))?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, json)
        .and_then(|()| std::fs::rename(&temp, path))
        .map_err(|e| failed(&e))
}

/// Values in the `encrypted_secrets` column, encrypted with the app key file.
pub struct FileSecretStore<'a> {
    repo: &'a dyn MCPServerRepository,
    key_path: PathBuf,
    /// Serializes the read-modify-write of one row's value map.
    write_lock: tokio::sync::Mutex<()>,
    /// Set when a save created the key file; the warning is reported by the first
    /// successful write after that. A restart before that write may lose the warning.
    pending_warning: std::sync::atomic::AtomicBool,
}

impl<'a> FileSecretStore<'a> {
    pub fn new(repo: &'a dyn MCPServerRepository, key_path: PathBuf) -> Self {
        Self {
            repo,
            key_path,
            write_lock: tokio::sync::Mutex::new(()),
            pending_warning: std::sync::atomic::AtomicBool::new(false),
        }
    }

    async fn column(&self, server_id: &str) -> Result<Option<String>, String> {
        let model = self
            .repo
            .get(server_id)
            .await
            .map_err(|e| format!("Failed to read MCP server '{server_id}': {e}"))?;
        Ok(model.and_then(|model| model.encrypted_secrets))
    }

    fn existing_key(&self) -> Result<[u8; KEY_LEN], String> {
        match read_key(&self.key_path)? {
            KeyFile::Key(key) => Ok(key),
            KeyFile::Missing => Err(format!(
                "The MCP secret key file {} is missing, so stored secret values cannot be read. Enter every secret again with tool__updateServer and reenterSecrets.",
                self.key_path.display()
            )),
            KeyFile::Damaged => Err(format!(
                "The MCP secret key file {} is damaged, so stored secret values cannot be read. Enter every secret again with tool__updateServer and reenterSecrets; a new key file is created.",
                self.key_path.display()
            )),
        }
    }

    /// The warning, shown once: the first successful write after the key file was
    /// created. A write that fails between the creation and the next save delays the
    /// warning instead of losing it.
    fn fallback_warning(&self) -> Option<String> {
        if !self
            .pending_warning
            .swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            return None;
        }
        log::warn!("{FALLBACK_WARNING}");
        Some(FALLBACK_WARNING.to_string())
    }

    async fn write_column(&self, server_id: &str, column: Option<String>) -> Result<(), String> {
        self.repo
            .set_encrypted_secrets(server_id, column)
            .await
            .map_err(|e| format!("Failed to save encrypted secrets: {e}"))
    }
}

#[async_trait::async_trait]
impl SecretStore for FileSecretStore<'_> {
    async fn get(&self, server_id: &str, name: &str) -> Result<Option<String>, String> {
        let Some(column) = self.column(server_id).await? else {
            return Ok(None);
        };
        let mut values = decrypt(&self.existing_key()?, server_id, &column)?;
        Ok(values.remove(name))
    }

    async fn set(
        &self,
        server_id: &str,
        name: &str,
        value: &str,
    ) -> Result<Option<String>, String> {
        let _guard = self.write_lock.lock().await;
        let (key, created) = load_or_create_key(&self.key_path)?;
        if created {
            self.pending_warning
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let mut values = match self.column(server_id).await? {
            // A lost or damaged key file leaves the stored values unreadable for good, so
            // the new value replaces them instead of failing on every retry.
            Some(column) => decrypt(&key, server_id, &column).unwrap_or_else(|error| {
                log::warn!(
                    "Replacing the secret values of MCP server '{server_id}' that cannot be read: {error}"
                );
                BTreeMap::new()
            }),
            None => BTreeMap::new(),
        };
        values.insert(name.to_string(), value.to_string());
        self.write_column(server_id, Some(encrypt(&key, server_id, &values)?))
            .await?;
        Ok(self.fallback_warning())
    }

    async fn delete(&self, server_id: &str, name: &str) -> Result<(), String> {
        let _guard = self.write_lock.lock().await;
        let Some(column) = self.column(server_id).await? else {
            return Ok(());
        };
        // A column that cannot be read is dropped rather than blocking the delete.
        let key = match read_key(&self.key_path)? {
            KeyFile::Key(key) => key,
            KeyFile::Missing | KeyFile::Damaged => {
                log::warn!(
                    "Clearing the secret values of MCP server '{server_id}': the key file {} cannot be used.",
                    self.key_path.display()
                );
                return self.write_column(server_id, None).await;
            }
        };
        let mut values = match decrypt(&key, server_id, &column) {
            Ok(values) => values,
            Err(error) => {
                log::warn!(
                    "Clearing the secret values of MCP server '{server_id}' that cannot be read: {error}"
                );
                return self.write_column(server_id, None).await;
            }
        };
        if values.remove(name).is_none() {
            return Ok(());
        }
        let column = if values.is_empty() {
            None
        } else {
            Some(encrypt(&key, server_id, &values)?)
        };
        self.write_column(server_id, column).await
    }
}

/// base64 of nonce + ciphertext. The server id is the associated data, so a value map
/// copied to another row does not decrypt.
fn encrypt(
    key: &[u8; KEY_LEN],
    server_id: &str,
    values: &BTreeMap<String, String>,
) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let plaintext = serde_json::to_vec(values).map_err(|e| e.to_string())?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: server_id.as_bytes(),
            },
        )
        .map_err(|_| "Failed to encrypt MCP secret values".to_string())?;
    let mut sealed = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(sealed))
}

fn decrypt(
    key: &[u8; KEY_LEN],
    server_id: &str,
    column: &str,
) -> Result<BTreeMap<String, String>, String> {
    let unreadable = || {
        format!("The encrypted secrets of MCP server '{server_id}' cannot be read. Enter them again with tool__updateServer and reenterSecrets.")
    };
    let sealed = STANDARD.decode(column).map_err(|_| unreadable())?;
    if sealed.len() <= NONCE_LEN {
        return Err(unreadable());
    }
    let (nonce, ciphertext) = sealed.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: server_id.as_bytes(),
            },
        )
        .map_err(|_| unreadable())?;
    serde_json::from_slice(&plaintext).map_err(|_| unreadable())
}

/// What the key file at `path` holds.
enum KeyFile {
    /// A usable app key.
    Key([u8; KEY_LEN]),
    /// No file: stored values cannot be read, but a new key may be created.
    Missing,
    /// A file that is not a key: stored values cannot be read again.
    Damaged,
}

fn read_key(path: &Path) -> Result<KeyFile, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(match <[u8; KEY_LEN]>::try_from(bytes.as_slice()) {
            Ok(key) => KeyFile::Key(key),
            Err(_) => KeyFile::Damaged,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(KeyFile::Missing),
        Err(e) => Err(format!(
            "Failed to read the MCP secret key file {}: {e}",
            path.display()
        )),
    }
}

/// The app key, created when there is none. A damaged key file is replaced, because it
/// can never decrypt the column.
///
/// The key is written to a private temporary file first and then hard-linked into
/// place, so a reader never sees a partial key and two processes never both win.
fn load_or_create_key(path: &Path) -> Result<([u8; KEY_LEN], bool), String> {
    match read_key(path)? {
        KeyFile::Key(key) => return Ok((key, false)),
        KeyFile::Missing => {}
        KeyFile::Damaged => {
            log::warn!(
                "Replacing the damaged MCP secret key file {}; earlier secret values cannot be read.",
                path.display()
            );
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(format!(
                        "Failed to replace the damaged MCP secret key file {}: {e}",
                        path.display()
                    ))
                }
            }
        }
    }
    let dir = path
        .parent()
        .ok_or_else(|| format!("Invalid key file path {}", path.display()))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("Failed to create {}: {e}", dir.display()))?;

    let mut key = [0u8; KEY_LEN];
    OsRng.fill_bytes(&mut key);
    let mut suffix = [0u8; 8];
    OsRng.fill_bytes(&mut suffix);
    let suffix: String = suffix.iter().map(|byte| format!("{byte:02x}")).collect();
    let temp = dir.join(format!(".{KEY_FILE_NAME}.{suffix}.tmp"));

    let linked = write_private_file(&temp, &key).and_then(|()| {
        std::fs::hard_link(&temp, path).map(|()| true).or_else(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(false)
            } else {
                Err(format!(
                    "Failed to create the MCP secret key file {}: {e}",
                    path.display()
                ))
            }
        })
    });
    let _ = std::fs::remove_file(&temp);
    if linked? {
        log::info!("Created the MCP secret key file {}", path.display());
        Ok((key, true))
    } else {
        match read_key(path)? {
            KeyFile::Key(key) => Ok((key, false)),
            KeyFile::Missing | KeyFile::Damaged => Err(format!(
                "The MCP secret key file {} was created by another process but cannot be read.",
                path.display()
            )),
        }
    }
}

/// Create `path` readable and writable by the current user only (Unix 0600; on Windows a
/// protected ACL that grants the current user's SID), then write `bytes`.
fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("Failed to create {}: {e}", path.display()))?;
    #[cfg(windows)]
    restrict_to_current_user(path)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

/// The current process user's SID in string form (for example `S-1-5-21-...`).
#[cfg(windows)]
fn current_user_sid() -> Result<String, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, HANDLE};
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let os_error = |what: &str| format!("{what}: {}", std::io::Error::last_os_error());
    // SAFETY: the token handle is closed once on every path after it is opened. The
    // TOKEN_USER buffer is u64-aligned and as large as GetTokenInformation asked for, and
    // the SID string allocated by ConvertSidToStringSidW is read up to its NUL and freed
    // once with LocalFree.
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(os_error("Failed to open the process token"));
        }
        let mut needed = 0u32;
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
        let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
        let read = GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        );
        let read_error = os_error("Failed to read the process user");
        CloseHandle(token);
        if read == 0 {
            return Err(read_error);
        }
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut sid_string: *mut u16 = std::ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut sid_string) == 0 {
            return Err(os_error("Failed to format the process user SID"));
        }
        let len = (0..).take_while(|&i| *sid_string.add(i) != 0).count();
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(sid_string, len));
        LocalFree(sid_string.cast());
        Ok(sid)
    }
}

#[cfg(windows)]
fn restrict_to_current_user(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{
        SetFileSecurityW, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR,
    };

    // Protected DACL: full access for the current user's SID, nothing inherited. The owner
    // alias (OW) is not used: under an elevated token the owner is Administrators, and a
    // later non-elevated run would lose access to the key.
    let sddl: Vec<u16> = format!("D:P(A;;FA;;;{})", current_user_sid()?)
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive the calls, and
    // the descriptor allocated by the first call is freed once with LocalFree.
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(format!(
                "Failed to build the key file ACL: {}",
                std::io::Error::last_os_error()
            ));
        }
        let applied = SetFileSecurityW(
            wide_path.as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        );
        let error = std::io::Error::last_os_error();
        LocalFree(descriptor);
        if applied == 0 {
            return Err(format!(
                "Failed to restrict {} to the current user: {error}",
                path.display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::secret_templates::MemorySecretStore;
    use crate::repositories::mcp_server_repository::SqliteMCPServerRepository;
    use sea_orm::Database;
    use sea_orm_migration::MigratorTrait;

    async fn repo() -> SqliteMCPServerRepository {
        crate::lifecycle::database::register_sqlite_vec();
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        crate::migration::Migrator::up(&db, None)
            .await
            .expect("migrations");
        SqliteMCPServerRepository::new(db)
    }

    async fn server(repo: &SqliteMCPServerRepository) -> String {
        repo.create(
            "exa",
            serde_json::json!({
                "name": "exa",
                "transport": {
                    "type": "http",
                    "url": "https://mcp.exa.ai/mcp?exaApiKey={{secret:EXA_API_KEY}}"
                }
            }),
        )
        .await
        .expect("create")
        .id
    }

    #[tokio::test]
    async fn without_a_keychain_values_are_encrypted_in_the_row() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let keychain = MemorySecretStore::unavailable();
        let store = LayeredSecretStore::new(
            &keychain,
            FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME)),
        );

        let notice = store.set(&id, "EXA_API_KEY", "exa-secret").await.unwrap();

        assert_eq!(notice.as_deref(), Some(FALLBACK_WARNING));
        let row = repo.get(&id).await.unwrap().expect("row");
        let column = row.encrypted_secrets.clone().expect("encrypted column");
        assert!(!column.contains("exa-secret"));
        assert!(!row.config.contains("exa-secret"));
        assert!(!format!("{row:?}").contains("exa-secret"));
        assert_eq!(
            store.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
            Some("exa-secret")
        );
        assert_eq!(keychain.get(&id, "EXA_API_KEY").await.unwrap(), None);
    }

    #[tokio::test]
    async fn the_key_file_is_created_once_private_and_reused() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join(KEY_FILE_NAME);
        let store = FileSecretStore::new(&repo, key_path.clone());

        let first = store.set(&id, "A_KEY", "one").await.unwrap();
        let key = std::fs::read(&key_path).unwrap();
        let second = store.set(&id, "B_KEY", "two").await.unwrap();
        let column_before = repo.get(&id).await.unwrap().unwrap().encrypted_secrets;
        store.set(&id, "B_KEY", "two").await.unwrap();
        let column_after = repo.get(&id).await.unwrap().unwrap().encrypted_secrets;

        assert_eq!(first.as_deref(), Some(FALLBACK_WARNING));
        assert_eq!(second, None, "the warning shows once");
        assert_eq!(key.len(), KEY_LEN);
        assert_eq!(std::fs::read(&key_path).unwrap(), key, "the key is reused");
        assert_ne!(column_before, column_after, "every write uses a new nonce");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&key_path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name() != KEY_FILE_NAME)
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");

        let reopened = FileSecretStore::new(&repo, key_path);
        assert_eq!(
            reopened.get(&id, "A_KEY").await.unwrap().as_deref(),
            Some("one")
        );
    }

    #[tokio::test]
    async fn deleting_the_last_value_clears_the_column() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let store = FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME));
        store.set(&id, "A_KEY", "one").await.unwrap();
        store.set(&id, "B_KEY", "two").await.unwrap();

        store.delete(&id, "A_KEY").await.unwrap();
        assert_eq!(store.get(&id, "A_KEY").await.unwrap(), None);
        assert_eq!(
            store.get(&id, "B_KEY").await.unwrap().as_deref(),
            Some("two")
        );

        store.delete(&id, "B_KEY").await.unwrap();
        let row = repo.get(&id).await.unwrap().unwrap();
        assert_eq!(row.encrypted_secrets, None);
    }

    #[tokio::test]
    async fn a_value_map_copied_to_another_row_does_not_decrypt() {
        let repo = repo().await;
        let id = server(&repo).await;
        let other = repo
            .create("other", serde_json::json!({ "name": "other", "transport": { "type": "http", "url": "https://a.example" } }))
            .await
            .unwrap()
            .id;
        let dir = tempfile::tempdir().unwrap();
        let store = FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME));
        store.set(&id, "A_KEY", "one").await.unwrap();
        let column = repo.get(&id).await.unwrap().unwrap().encrypted_secrets;
        repo.set_encrypted_secrets(&other, column).await.unwrap();

        let error = store.get(&other, "A_KEY").await.err().expect("fails");
        assert!(error.contains("reenterSecrets"), "{error}");
    }

    #[tokio::test]
    async fn a_working_keychain_is_used_and_the_row_stays_empty() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let keychain = MemorySecretStore::default();
        let store = LayeredSecretStore::new(
            &keychain,
            FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME)),
        );

        assert_eq!(store.set(&id, "A_KEY", "one").await.unwrap(), None);

        assert_eq!(
            keychain.get(&id, "A_KEY").await.unwrap().as_deref(),
            Some("one")
        );
        assert_eq!(
            repo.get(&id).await.unwrap().unwrap().encrypted_secrets,
            None
        );
        assert!(!dir.path().join(KEY_FILE_NAME).exists());
    }

    #[tokio::test]
    async fn a_keychain_save_removes_the_older_fallback_copy() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join(KEY_FILE_NAME);
        let fallback = FileSecretStore::new(&repo, key_path.clone());
        fallback.set(&id, "A_KEY", "old").await.unwrap();
        fallback.set(&id, "B_KEY", "kept").await.unwrap();

        let keychain = MemorySecretStore::default();
        let store = LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path));
        store.set(&id, "A_KEY", "new").await.unwrap();

        assert_eq!(
            keychain.get(&id, "A_KEY").await.unwrap().as_deref(),
            Some("new")
        );
        assert_eq!(fallback.get(&id, "A_KEY").await.unwrap(), None);
        assert_eq!(
            fallback.get(&id, "B_KEY").await.unwrap().as_deref(),
            Some("kept")
        );
    }

    #[tokio::test]
    async fn a_lost_key_file_still_accepts_a_new_value() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join(KEY_FILE_NAME);
        let store = FileSecretStore::new(&repo, key_path.clone());
        store.set(&id, "EXA_API_KEY", "old").await.unwrap();

        std::fs::remove_file(&key_path).unwrap();
        assert!(
            store.get(&id, "EXA_API_KEY").await.is_err(),
            "the old value is gone with the key file"
        );

        // The key file is recreated here, so the pending warning is armed again: the
        // stored values of every row were replaced with unreadable ones.
        assert_eq!(
            store
                .set(&id, "EXA_API_KEY", "new")
                .await
                .unwrap()
                .as_deref(),
            Some(FALLBACK_WARNING)
        );
        assert_eq!(
            store.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
            Some("new")
        );
        assert_eq!(std::fs::read(&key_path).unwrap().len(), KEY_LEN);
    }

    #[tokio::test]
    async fn a_damaged_key_file_is_replaced() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join(KEY_FILE_NAME);
        let store = FileSecretStore::new(&repo, key_path.clone());
        store.set(&id, "EXA_API_KEY", "old").await.unwrap();
        std::fs::write(&key_path, b"not-a-key").unwrap();

        assert!(
            store.get(&id, "EXA_API_KEY").await.is_err(),
            "a damaged key cannot read the column"
        );
        store.set(&id, "EXA_API_KEY", "new").await.unwrap();
        assert_eq!(
            store.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
            Some("new")
        );
        assert_eq!(std::fs::read(&key_path).unwrap().len(), KEY_LEN);
    }

    #[tokio::test]
    async fn a_key_file_that_cannot_be_read_still_fails_the_write() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join(KEY_FILE_NAME);
        // A directory is never a key, and reading it fails with an error that is not
        // NotFound. That must not be treated as a missing key file.
        std::fs::create_dir(&key_path).unwrap();
        let store = FileSecretStore::new(&repo, key_path);

        let error = store.set(&id, "EXA_API_KEY", "value").await.err();
        assert!(error.is_some(), "a real read error stops the write");
    }

    #[tokio::test]
    async fn the_warning_shows_once_across_stores() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join(KEY_FILE_NAME);

        let first = FileSecretStore::new(&repo, key_path.clone());
        assert_eq!(
            first.set(&id, "A_KEY", "one").await.unwrap().as_deref(),
            Some(FALLBACK_WARNING)
        );

        // A restarted app builds a new store on the same key file; the warning was
        // already shown.
        let reopened = FileSecretStore::new(&repo, key_path);
        assert_eq!(reopened.set(&id, "B_KEY", "two").await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_value_rotated_during_an_outage_wins_over_the_stale_keychain_copy() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join(KEY_FILE_NAME);
        let keychain = MemorySecretStore::default();
        let store =
            LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path.clone()));
        store.set(&id, "EXA_API_KEY", "old").await.unwrap();

        keychain.set_available(false);
        store.set(&id, "EXA_API_KEY", "rotated").await.unwrap();
        assert_eq!(
            keychain.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
            Some("old"),
            "the keychain still holds the stale value"
        );

        // A restart with the keychain back.
        keychain.set_available(true);
        let restarted = LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path));
        assert_eq!(
            restarted.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
            Some("rotated")
        );
        assert_eq!(
            restarted.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
            Some("rotated")
        );
    }

    #[tokio::test]
    async fn a_fallback_value_is_moved_to_the_keychain_not_copied() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let keychain = MemorySecretStore::unavailable();
        let store = LayeredSecretStore::new(
            &keychain,
            FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME)),
        );
        store.set(&id, "EXA_API_KEY", "value").await.unwrap();

        keychain.set_available(true);
        assert_eq!(
            store.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
            Some("value")
        );

        assert_eq!(
            keychain.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
            Some("value")
        );
        assert_eq!(
            repo.get(&id).await.unwrap().unwrap().encrypted_secrets,
            None,
            "the fallback copy is cleared"
        );
    }

    /// A keychain that reports itself available but rejects writes, like a locked one.
    struct LockedKeychain;

    #[async_trait::async_trait]
    impl SecretStore for LockedKeychain {
        async fn get(&self, _: &str, _: &str) -> Result<Option<String>, String> {
            Ok(None)
        }
        async fn set(&self, _: &str, _: &str, _: &str) -> Result<Option<String>, String> {
            Err("the keychain is locked".to_string())
        }
        async fn delete(&self, _: &str, _: &str) -> Result<(), String> {
            Err("the keychain is locked".to_string())
        }
    }

    #[tokio::test]
    async fn a_keychain_that_rejects_the_move_keeps_the_fallback_copy() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join(KEY_FILE_NAME);
        FileSecretStore::new(&repo, key_path.clone())
            .set(&id, "EXA_API_KEY", "value")
            .await
            .unwrap();

        let store = LayeredSecretStore::new(&LockedKeychain, FileSecretStore::new(&repo, key_path));
        assert_eq!(
            store.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
            Some("value"),
            "the read still succeeds"
        );
        assert!(repo
            .get(&id)
            .await
            .unwrap()
            .unwrap()
            .encrypted_secrets
            .is_some());
    }

    #[tokio::test]
    async fn without_a_keychain_deleting_the_last_value_clears_the_column_and_creates_no_key() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join(KEY_FILE_NAME);
        let keychain = MemorySecretStore::unavailable();
        let store =
            LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path.clone()));

        store.delete(&id, "EXA_API_KEY").await.unwrap();
        assert!(!key_path.exists(), "a delete does not create the key file");

        store.set(&id, "EXA_API_KEY", "value").await.unwrap();
        store.delete(&id, "EXA_API_KEY").await.unwrap();
        assert_eq!(
            repo.get(&id).await.unwrap().unwrap().encrypted_secrets,
            None
        );
    }

    #[tokio::test]
    async fn a_delete_during_an_outage_reaches_the_keychain_without_a_read_of_that_name() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join(KEY_FILE_NAME);
        let keychain = MemorySecretStore::default();
        let store =
            LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path.clone()));
        store.set(&id, "EXA_API_KEY", "value").await.unwrap();

        keychain.set_available(false);
        store.delete(&id, "EXA_API_KEY").await.unwrap();
        assert_eq!(store.get(&id, "EXA_API_KEY").await.unwrap(), None);
        // The row goes away with the server; the pending delete does not.
        repo.delete(&id).await.unwrap();

        // A restart with the keychain back; any operation applies the pending delete.
        keychain.set_available(true);
        let restarted = LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path));
        assert_eq!(restarted.get("other", "OTHER_KEY").await.unwrap(), None);
        assert_eq!(keychain.get(&id, "EXA_API_KEY").await.unwrap(), None);
        assert!(!dir.path().join(PENDING_DELETES_FILE_NAME).exists());
    }

    #[tokio::test]
    async fn a_value_saved_after_an_outage_delete_is_not_removed_by_it() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let keychain = MemorySecretStore::default();
        let store = LayeredSecretStore::new(
            &keychain,
            FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME)),
        );
        store.set(&id, "EXA_API_KEY", "old").await.unwrap();

        keychain.set_available(false);
        store.delete(&id, "EXA_API_KEY").await.unwrap();
        store.set(&id, "EXA_API_KEY", "new").await.unwrap();

        keychain.set_available(true);
        assert_eq!(
            store.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
            Some("new")
        );
        store.set(&id, "B_KEY", "other").await.unwrap();
        assert_eq!(
            keychain.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
            Some("new")
        );
    }

    /// A keychain that saves values but cannot delete them.
    #[derive(Default)]
    struct UndeletableKeychain(MemorySecretStore);

    #[async_trait::async_trait]
    impl SecretStore for UndeletableKeychain {
        async fn get(&self, server_id: &str, name: &str) -> Result<Option<String>, String> {
            self.0.get(server_id, name).await
        }
        async fn set(
            &self,
            server_id: &str,
            name: &str,
            value: &str,
        ) -> Result<Option<String>, String> {
            self.0.set(server_id, name, value).await
        }
        async fn delete(&self, _: &str, _: &str) -> Result<(), String> {
            Err("the keychain rejected the delete".to_string())
        }
    }

    #[tokio::test]
    async fn a_failed_pending_delete_does_not_remove_a_value_saved_later() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join(KEY_FILE_NAME);
        LayeredSecretStore::new(
            &MemorySecretStore::unavailable(),
            FileSecretStore::new(&repo, key_path.clone()),
        )
        .delete(&id, "EXA_API_KEY")
        .await
        .unwrap();

        let keychain = UndeletableKeychain::default();
        let store = LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path));
        store.set(&id, "EXA_API_KEY", "new").await.unwrap();

        assert!(
            !dir.path().join(PENDING_DELETES_FILE_NAME).exists(),
            "the save drops the pending delete of the same name"
        );
        assert_eq!(
            keychain.get(&id, "EXA_API_KEY").await.unwrap().as_deref(),
            Some("new")
        );
    }

    #[tokio::test]
    async fn an_unreadable_pending_list_still_clears_the_fallback_copy() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let keychain = MemorySecretStore::unavailable();
        let store = LayeredSecretStore::new(
            &keychain,
            FileSecretStore::new(&repo, dir.path().join(KEY_FILE_NAME)),
        );
        store.set(&id, "EXA_API_KEY", "value").await.unwrap();
        std::fs::write(dir.path().join(PENDING_DELETES_FILE_NAME), b"not json").unwrap();

        assert!(store.delete(&id, "EXA_API_KEY").await.is_err());
        assert_eq!(
            repo.get(&id).await.unwrap().unwrap().encrypted_secrets,
            None
        );
    }

    #[tokio::test]
    async fn a_pending_delete_that_fails_stays_for_the_next_try() {
        let repo = repo().await;
        let id = server(&repo).await;
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join(KEY_FILE_NAME);
        let keychain = MemorySecretStore::unavailable();
        LayeredSecretStore::new(&keychain, FileSecretStore::new(&repo, key_path.clone()))
            .delete(&id, "EXA_API_KEY")
            .await
            .unwrap();

        let locked =
            LayeredSecretStore::new(&LockedKeychain, FileSecretStore::new(&repo, key_path));
        assert_eq!(locked.get(&id, "B_KEY").await.unwrap(), None);
        assert!(dir.path().join(PENDING_DELETES_FILE_NAME).exists());
    }
}
