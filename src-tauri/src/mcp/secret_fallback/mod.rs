//! Keychain-unavailable fallback for `{{secret:NAME}}` values (#2570).
//!
//! When the OS keychain does not work, the `{NAME: value}` map of a server is encrypted
//! with AES-256-GCM and stored in the `encrypted_secrets` column of its `mcp_servers`
//! row. The key is a random 256-bit app key in a file next to the database, readable by
//! the current user only. The key file is never exported.

mod file_store;
mod key_file;
mod layered_store;
mod pending_deletes;
mod private_file;

pub use file_store::{FileSecretStore, FALLBACK_WARNING};
pub(crate) use key_file::KEY_FILE_NAME;
pub use layered_store::LayeredSecretStore;
use pending_deletes::{read_pending_deletes, PENDING_DELETES_FILE_NAME};
use std::path::PathBuf;

/// The app key file in the app local data dir (next to the database).
pub fn app_key_path() -> PathBuf {
    crate::profile::data_dir(crate::profile::resolve_profile()).join(KEY_FILE_NAME)
}

/// The pending-deletes file in the app local data dir (next to the key file).
pub fn app_pending_deletes_path() -> PathBuf {
    app_key_path().with_file_name(PENDING_DELETES_FILE_NAME)
}

/// At app start, if `mcp-secrets-pending-deletes.json` exists and is not empty, run
/// the pending-delete sweep once.
pub async fn sweep_pending_deletes_at_startup() {
    let path = app_pending_deletes_path();
    if !path.is_file() {
        return;
    }
    let is_empty = match std::fs::metadata(&path) {
        Ok(m) if m.len() == 0 => true,
        _ => match read_pending_deletes(&path) {
            Ok(pending) => pending.is_empty(),
            Err(_) => false,
        },
    };
    if is_empty {
        return;
    }
    crate::mcp::secret_templates::secret_store()
        .sweep_pending_deletes_at_startup()
        .await;
}

#[cfg(test)]
mod tests;
