use super::sources::{get_source_in, validate_source_id};
use super::{enabled_by_default, MANIFEST_FILE};
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Approval state of one session-installed plugin, stored outside the
/// session workspace (see [`super::SESSION_PLUGIN_TRUST_DIR_NAME`]). This is the
/// only authority for hook and MCP-server approval: the workspace
/// manifest's own approval fields are scrubbed at install and ignored at
/// load (#2942 S3.2 carries `mcp_approved` here too, so the store moving
/// into the agent-writable workspace cannot weaken #2924 D3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionPluginTrust {
    #[serde(default)]
    pub hooks_approved: bool,
    #[serde(default)]
    pub approval_hashes: BTreeMap<String, String>,
    /// Whether the user approved starting this plugin's MCP servers
    /// (#2924 D3, carried out of the workspace store by #2942 S3.2). Same
    /// rule as hooks: the workspace manifest's own flag is scrubbed at
    /// install and ignored at load; only this record can approve.
    #[serde(default)]
    pub mcp_approved: bool,
    /// Whether the user turned this session plugin off (#2927). The state
    /// lives here for the same reason as the approvals: the workspace
    /// manifest is agent-writable, so a plugin the user disabled could
    /// otherwise be turned back on by an edit (`"enabled": true`) and its
    /// hooks, commands and MCP servers would start again. Missing from
    /// records written before this field existed, and a plugin is then
    /// enabled — the state it had before #2927.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

impl Default for SessionPluginTrust {
    /// No record means no approvals, but still enabled: being enabled is the
    /// plugin's starting state, not a privilege that approval grants, and a
    /// missing or unreadable record must not silently turn a plugin off.
    fn default() -> Self {
        Self {
            hooks_approved: false,
            approval_hashes: BTreeMap::new(),
            mcp_approved: false,
            enabled: true,
        }
    }
}

fn trust_file_path(trust_dir: &Path, source_id: &str) -> PathBuf {
    trust_dir.join(format!("{source_id}.json"))
}

/// Persist one plugin's approval state into the trust store (install path).
pub fn record_session_plugin_trust(
    trust_dir: &Path,
    source_id: &str,
    trust: &SessionPluginTrust,
) -> Result<(), String> {
    validate_source_id(source_id)?;
    fs::create_dir_all(trust_dir).map_err(|e| format!("create {}: {e}", trust_dir.display()))?;
    let path = trust_file_path(trust_dir, source_id);
    let json = serde_json::to_string_pretty(trust).map_err(|e| format!("serialize trust: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Read one plugin's trust record. A missing or unreadable file means
/// unapproved with no hashes — fail-safe (gatekeeper G1.1).
pub fn read_session_plugin_trust(trust_dir: &Path, source_id: &str) -> SessionPluginTrust {
    let path = trust_file_path(trust_dir, source_id);
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|error| {
            warn!("Ignoring unreadable {}: {}", path.display(), error);
            SessionPluginTrust::default()
        }),
        Err(_) => SessionPluginTrust::default(),
    }
}

/// Delete one plugin's trust record (uninstall). A missing file is fine.
pub fn remove_session_plugin_trust(trust_dir: &Path, source_id: &str) -> Result<(), String> {
    validate_source_id(source_id)?;
    let path = trust_file_path(trust_dir, source_id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("remove {}: {e}", path.display())),
    }
}

/// Move a just-installed session plugin's approval data out of the
/// workspace manifest into the trust store, then scrub the manifest's
/// approval fields (#2942 gatekeeper G1.1). After this, nothing inside the
/// session workspace can grant hook approval.
pub fn move_session_approval_to_trust(
    trust_dir: &Path,
    plugins_dir: &Path,
    source_id: &str,
) -> Result<(), String> {
    let record = get_source_in(plugins_dir, source_id)?;
    let trust = SessionPluginTrust {
        hooks_approved: record.manifest.hooks_approved,
        approval_hashes: record.manifest.approval_hashes.clone(),
        mcp_approved: record.manifest.mcp_approved,
        // #2927: the install computed the state the user wants (kept on
        // update, `defaultEnabled` on a fresh install); the trust record
        // becomes its only authority from here on.
        enabled: record.manifest.enabled,
    };
    record_session_plugin_trust(trust_dir, source_id, &trust)?;
    let mut scrubbed = record.manifest;
    scrubbed.hooks_approved = false;
    scrubbed.approval_hashes.clear();
    scrubbed.mcp_approved = false;
    let manifest_path = plugins_dir.join(source_id).join(MANIFEST_FILE);
    let json =
        serde_json::to_string_pretty(&scrubbed).map_err(|e| format!("serialize manifest: {e}"))?;
    fs::write(&manifest_path, json).map_err(|e| format!("write {}: {e}", manifest_path.display()))
}
