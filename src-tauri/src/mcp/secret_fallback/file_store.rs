//! The fallback store: values encrypted into the `encrypted_secrets` column.

use super::key_file::{load_or_create_key, read_key, KeyFile, KEY_LEN};
use crate::mcp::secret_templates::SecretStore;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use aes_gcm::aead::{rand_core::RngCore, Aead, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::collections::BTreeMap;
use std::path::PathBuf;

const NONCE_LEN: usize = 12;

/// Shown once, when the key file is created.
pub const FALLBACK_WARNING: &str = "Your OS keychain is not available. MCP secret values are encrypted with a key file on this computer. This protects them if only the database leaks, not if someone has the whole disk.";

/// Values in the `encrypted_secrets` column, encrypted with the app key file.
pub struct FileSecretStore<'a> {
    repo: &'a dyn MCPServerRepository,
    pub(super) key_path: PathBuf,
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
