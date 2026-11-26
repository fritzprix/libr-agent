//! Append-only session plugin install/uninstall history (`history.json`).

use super::SessionPluginHistoryEntry;
use log::warn;
use std::fs;
use std::path::Path;

const HISTORY_FILE: &str = "history.json";

pub(super) fn read_history(plugins_dir: &Path) -> Vec<SessionPluginHistoryEntry> {
    let path = plugins_dir.join(HISTORY_FILE);
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_else(|error| {
        warn!("Ignoring unreadable {}: {}", path.display(), error);
        Vec::new()
    })
}

/// Best-effort: a failed history write must not fail the install itself.
pub(super) fn append_history(plugins_dir: &Path, entry: SessionPluginHistoryEntry) {
    let mut history = read_history(plugins_dir);
    history.push(entry);
    let write = fs::create_dir_all(plugins_dir)
        .map_err(|e| e.to_string())
        .and_then(|_| serde_json::to_string_pretty(&history).map_err(|e| e.to_string()))
        .and_then(|json| {
            fs::write(plugins_dir.join(HISTORY_FILE), json).map_err(|e| e.to_string())
        });
    if let Err(error) = write {
        warn!("Failed to record session plugin history: {}", error);
    }
}
