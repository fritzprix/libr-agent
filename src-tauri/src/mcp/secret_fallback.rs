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
use std::collections::BTreeMap;
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

/// Uses `primary` (the OS keychain) when it is available, and `fallback` otherwise.
/// Values written while the keychain was unavailable stay readable after it comes back.
pub struct LayeredSecretStore<'a> {
    primary: &'a dyn SecretStore,
    fallback: FileSecretStore<'a>,
}

impl<'a> LayeredSecretStore<'a> {
    pub fn new(primary: &'a dyn SecretStore, fallback: FileSecretStore<'a>) -> Self {
        Self { primary, fallback }
    }
}

#[async_trait::async_trait]
impl SecretStore for LayeredSecretStore<'_> {
    async fn get(&self, server_id: &str, name: &str) -> Result<Option<String>, String> {
        if self.primary.available() {
            if let Some(value) = self.primary.get(server_id, name).await? {
                return Ok(Some(value));
            }
        }
        self.fallback.get(server_id, name).await
    }

    async fn set(
        &self,
        server_id: &str,
        name: &str,
        value: &str,
    ) -> Result<Option<String>, String> {
        if self.primary.available() {
            self.primary.set(server_id, name, value).await
        } else {
            self.fallback.set(server_id, name, value).await
        }
    }

    async fn delete(&self, server_id: &str, name: &str) -> Result<(), String> {
        if self.primary.available() {
            self.primary.delete(server_id, name).await?;
        }
        self.fallback.delete(server_id, name).await
    }
}

/// Values in the `encrypted_secrets` column, encrypted with the app key file.
pub struct FileSecretStore<'a> {
    repo: &'a dyn MCPServerRepository,
    key_path: PathBuf,
    /// Serializes the read-modify-write of one row's value map.
    write_lock: tokio::sync::Mutex<()>,
}

impl<'a> FileSecretStore<'a> {
    pub fn new(repo: &'a dyn MCPServerRepository, key_path: PathBuf) -> Self {
        Self {
            repo,
            key_path,
            write_lock: tokio::sync::Mutex::new(()),
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
        read_key(&self.key_path)?.ok_or_else(|| {
            format!(
                "The MCP secret key file {} is missing, so stored secret values cannot be read. Enter them again with tool__updateServer and reenterSecrets.",
                self.key_path.display()
            )
        })
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
        let mut values = match self.column(server_id).await? {
            Some(column) => decrypt(&key, server_id, &column)?,
            None => BTreeMap::new(),
        };
        values.insert(name.to_string(), value.to_string());
        self.write_column(server_id, Some(encrypt(&key, server_id, &values)?))
            .await?;
        if created {
            log::warn!("{FALLBACK_WARNING}");
        }
        Ok(created.then(|| FALLBACK_WARNING.to_string()))
    }

    async fn delete(&self, server_id: &str, name: &str) -> Result<(), String> {
        let _guard = self.write_lock.lock().await;
        let Some(column) = self.column(server_id).await? else {
            return Ok(());
        };
        let key = self.existing_key()?;
        let mut values = decrypt(&key, server_id, &column)?;
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

fn read_key(path: &Path) -> Result<Option<[u8; KEY_LEN]>, String> {
    match std::fs::read(path) {
        Ok(bytes) => <[u8; KEY_LEN]>::try_from(bytes.as_slice())
            .map(Some)
            .map_err(|_| format!("The MCP secret key file {} is damaged.", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!(
            "Failed to read the MCP secret key file {}: {e}",
            path.display()
        )),
    }
}

/// The app key, created on first need. Returns `true` when this call created it.
///
/// The key is written to a private temporary file first and then hard-linked into
/// place, so a reader never sees a partial key and two processes never both win.
fn load_or_create_key(path: &Path) -> Result<([u8; KEY_LEN], bool), String> {
    if let Some(key) = read_key(path)? {
        return Ok((key, false));
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
        Ok((key, true))
    } else {
        let key = read_key(path)?
            .ok_or_else(|| format!("The MCP secret key file {} vanished.", path.display()))?;
        Ok((key, false))
    }
}

/// Create `path` readable and writable by the current user only (Unix 0600; Windows a
/// protected ACL for the owner), then write `bytes`.
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

    // Protected DACL: full access for the owner (the current user), nothing inherited.
    let sddl: Vec<u16> = "D:P(A;;FA;;;OW)".encode_utf16().chain(Some(0)).collect();
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
}
