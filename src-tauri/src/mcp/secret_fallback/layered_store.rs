//! The keychain-first store that falls back to [`FileSecretStore`].

use super::file_store::FileSecretStore;
use super::pending_deletes::{
    read_pending_deletes, write_pending_deletes, PendingDeletes, PENDING_DELETES_FILE_NAME,
};
use crate::mcp::secret_templates::SecretStore;
use std::path::PathBuf;

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

    /// Runs the pending-delete sweep at startup if any deletes are pending and the
    /// primary store is available.
    pub async fn sweep_pending_deletes_at_startup(&self) {
        if !self.pending_deletes_path.is_file() {
            return;
        }
        let is_empty = match std::fs::metadata(&self.pending_deletes_path) {
            Ok(m) if m.len() == 0 => true,
            _ => match read_pending_deletes(&self.pending_deletes_path) {
                Ok(pending) => pending.is_empty(),
                Err(_) => false,
            },
        };
        if is_empty {
            return;
        }
        if !self.primary.available() {
            return;
        }
        self.apply_pending_deletes().await;
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

    async fn sweep_pending_deletes_at_startup(&self) {
        LayeredSecretStore::sweep_pending_deletes_at_startup(self).await;
    }
}
