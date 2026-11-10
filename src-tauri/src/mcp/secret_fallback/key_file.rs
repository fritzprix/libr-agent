//! The app key file that encrypts the fallback secret values.

use super::private_file::write_private_file;
use aes_gcm::aead::{rand_core::RngCore, OsRng};
use std::path::Path;

pub(crate) const KEY_FILE_NAME: &str = "mcp-secrets.key";
pub(super) const KEY_LEN: usize = 32;

/// What the key file at `path` holds.
pub(super) enum KeyFile {
    /// A usable app key.
    Key([u8; KEY_LEN]),
    /// No file: stored values cannot be read, but a new key may be created.
    Missing,
    /// A file that is not a key: stored values cannot be read again.
    Damaged,
}

pub(super) fn read_key(path: &Path) -> Result<KeyFile, String> {
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
pub(super) fn load_or_create_key(path: &Path) -> Result<([u8; KEY_LEN], bool), String> {
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
