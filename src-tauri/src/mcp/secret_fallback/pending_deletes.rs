//! Keychain deletes recorded during an outage, kept in a file next to the key file.

use std::collections::BTreeSet;
use std::path::Path;

/// Keychain entries deleted while the keychain was unavailable, next to the key file.
pub(super) const PENDING_DELETES_FILE_NAME: &str = "mcp-secrets-pending-deletes.json";

/// `(server_id, name)` pairs whose keychain entry must still be removed.
pub(super) type PendingDeletes = BTreeSet<(String, String)>;

pub(super) fn read_pending_deletes(path: &Path) -> Result<PendingDeletes, String> {
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
pub(super) fn write_pending_deletes(path: &Path, pending: &PendingDeletes) -> Result<(), String> {
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
