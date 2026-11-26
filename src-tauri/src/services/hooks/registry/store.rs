use super::sources::get_source_in;
use super::trust::read_session_plugin_trust;
use super::PluginSourceRecord;
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// One entry of the session plugin store manifest (#2942, owner D1): every
/// plugin installed into a session's plugin directory is listed here, and
/// session load reads plugins from this manifest instead of scanning the
/// directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPluginStoreEntry {
    pub source_id: String,
    pub source_url: String,
    /// Unix seconds.
    pub installed_at: i64,
}

/// File name of the session plugin store manifest inside the session plugin
/// directory (`workspaces/<session_id>/.libragent/plugins/store-manifest.json`
/// — the default session workspace directory, #2942 D1 revised).
pub const SESSION_STORE_MANIFEST_FILE: &str = "store-manifest.json";

/// Read the session plugin store manifest. A missing or unreadable file
/// yields an empty list (with a log line) — never a load failure (#2942 D1).
pub fn read_store_manifest(plugins_dir: &Path) -> Vec<SessionPluginStoreEntry> {
    let path = plugins_dir.join(SESSION_STORE_MANIFEST_FILE);
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_else(|error| {
        warn!("Ignoring unreadable {}: {}", path.display(), error);
        Vec::new()
    })
}

/// Rewrite the session plugin store manifest. Callers must treat an error
/// as a failed operation (#2942 gatekeeper G1.2): an installed plugin that
/// is not listed here is never loaded, so reporting success would be a lie.
fn write_store_manifest(
    plugins_dir: &Path,
    entries: &[SessionPluginStoreEntry],
) -> Result<(), String> {
    fs::create_dir_all(plugins_dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    fs::write(plugins_dir.join(SESSION_STORE_MANIFEST_FILE), json).map_err(|e| e.to_string())
}

/// Record (or refresh) one plugin in the session plugin store manifest.
pub fn upsert_session_store_entry(
    plugins_dir: &Path,
    entry: SessionPluginStoreEntry,
) -> Result<(), String> {
    let mut entries = read_store_manifest(plugins_dir);
    match entries.iter_mut().find(|e| e.source_id == entry.source_id) {
        Some(existing) => *existing = entry,
        None => entries.push(entry),
    }
    write_store_manifest(plugins_dir, &entries)
}

/// Remove one plugin from the session plugin store manifest (uninstall).
pub fn remove_session_store_entry(plugins_dir: &Path, source_id: &str) -> Result<(), String> {
    let mut entries = read_store_manifest(plugins_dir);
    let before = entries.len();
    entries.retain(|e| e.source_id != source_id);
    if entries.len() != before {
        write_store_manifest(plugins_dir, &entries)?;
    }
    Ok(())
}

/// Session plugin records resolved through the store manifest (#2942).
/// An entry whose plugin directory was deleted or whose manifest is broken
/// is skipped with a log line; the caller's load never fails (#2942 D1
/// failure policy). This is the manifest-driven replacement for
/// [`super::list_sources_in`] on a session plugin directory.
///
/// Approval is overlaid from the out-of-workspace trust store
/// (`trust_dir`, see [`super::SESSION_PLUGIN_TRUST_DIR_NAME`]); the workspace
/// manifest's own approval fields are ignored, and a plugin with no trust
/// record loads as unapproved (#2942 gatekeeper G1.1). `trust_dir: None`
/// means "no trust can exist": every session plugin loads unapproved.
pub fn list_session_store_sources(
    trust_dir: Option<&Path>,
    plugins_dir: &Path,
) -> Vec<PluginSourceRecord> {
    read_store_manifest(plugins_dir)
        .into_iter()
        .filter_map(|entry| match get_source_in(plugins_dir, &entry.source_id) {
            Ok(mut record) => {
                let trust = trust_dir
                    .map(|dir| read_session_plugin_trust(dir, &entry.source_id))
                    .unwrap_or_default();
                record.manifest.hooks_approved = trust.hooks_approved;
                record.manifest.approval_hashes = trust.approval_hashes;
                record.manifest.mcp_approved = trust.mcp_approved;
                // #2927: the enabled state is trust data too, so a workspace
                // manifest edited to `"enabled": true` cannot turn a plugin
                // the user disabled back on.
                record.manifest.enabled = trust.enabled;
                Some(record)
            }
            Err(error) => {
                warn!(
                    "Skipping session plugin '{}' ({}): {}; session load continues",
                    entry.source_id,
                    plugins_dir.display(),
                    error
                );
                None
            }
        })
        .collect()
}
