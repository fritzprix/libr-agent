mod approval;
mod dirs;
mod sources;
mod store;
mod trust;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(crate) const MANIFEST_FILE: &str = "manifest.json";

pub use approval::{
    approval_hash_key, approval_hash_value, compute_handler_approval_hashes, compute_script_hash,
    list_approved_plugin_roots, list_approved_sources_in, list_effective_approved_sources_in,
    set_hooks_approved,
};
pub use dirs::{
    ensure_plugin_data_dir, get_session_plugin_data_dir, get_session_plugin_env_file,
    get_session_plugin_trust_dir, get_session_plugins_directory, get_user_plugin_data_dir,
    get_user_plugins_directory, session_store_dir_in, LEGACY_SESSION_PLUGINS_DIR_NAME,
    PLUGIN_DATA_DIR_NAME, PLUGIN_ENV_DIR_NAME, PLUGIN_ENV_FILE_NAME, SESSION_PLUGIN_TRUST_DIR_NAME,
    SESSION_WORKSPACE_PLUGINS_DIR, USER_PLUGINS_DIR_NAME,
};
pub use sources::{
    delete_source, delete_source_in, get_source, get_source_in, hook_config_for_record,
    list_sources, list_sources_in, save_source, save_source_in, set_session_source_enabled_in,
    set_source_enabled_in, validate_source_id,
};
pub use store::{
    list_session_store_sources, read_store_manifest, remove_session_store_entry,
    upsert_session_store_entry, SessionPluginStoreEntry, SESSION_STORE_MANIFEST_FILE,
};
pub use trust::{
    move_session_approval_to_trust, read_session_plugin_trust, record_session_plugin_trust,
    remove_session_plugin_trust, SessionPluginTrust,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSourceManifest {
    pub source_url: String,
    pub source_id: String,
    pub skill_names: Vec<String>,
    pub hooks: Option<HooksConfigSnapshot>,
    pub hooks_approved: bool,
    pub approval_hashes: BTreeMap<String, String>,
    pub created_at: i64,
    /// Metadata parsed from `.claude-plugin/plugin.json` (#2921). `None` for
    /// sources installed without a manifest (including all pre-#2921 stores).
    #[serde(default)]
    pub plugin: Option<PluginMetadata>,
    /// Hook config files, relative to the installed root, in load order:
    /// the auto-loaded `hooks/hooks.json` / `hooks-cursor.json` that
    /// existed at install plus the files the manifest's `hooks` field
    /// declares (#2921). Install persists this list so dispatch and the
    /// Skills overview load exactly the config the user approved, instead
    /// of re-discovering only the defaults (gatekeeper G2.2). Empty on
    /// stores recorded before the list existed: fall back to default
    /// discovery, which is what those installs could only have had.
    #[serde(default)]
    pub hook_files: Vec<String>,
    /// Inline hooks objects the manifest declared instead of, or alongside,
    /// hook config files (#3005). Stored as the raw declared JSON so dispatch
    /// loads exactly the config the user approved, and so a plugin without a
    /// `hooks/` directory can still ship hooks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inline_hooks: Vec<serde_json::Value>,
    /// MCP servers the plugin declared (`.mcp.json` + manifest
    /// `mcpServers`, #2924), stored as the raw declared entries so session
    /// load can resolve `${CLAUDE_PLUGIN_ROOT}` against this record's
    /// installed root. Empty on stores recorded before #2924.
    #[serde(default)]
    pub mcp_servers: Vec<PluginMcpServerSnapshot>,
    /// Whether the user approved starting these servers at install (D3:
    /// one-time approval, same gate as hooks).
    #[serde(default)]
    pub mcp_approved: bool,
    /// Whether the plugin is enabled (#2927). A disabled plugin keeps its
    /// files but contributes no skills, hooks, commands, or MCP servers.
    /// Stores recorded before #2927 have no field and load as enabled, so an
    /// upgrade never disables a working plugin.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    /// The source commit the plugin was installed from (#2927), when it is
    /// known: GitHub serves an API zipball as `<owner>-<repo>-<sha>`, so the
    /// archive root name carries it. `None` for a branch archive or a store
    /// recorded before #2927.
    #[serde(default)]
    pub installed_commit: Option<String>,
}

pub(crate) fn enabled_by_default() -> bool {
    true
}

/// One plugin-declared MCP server as declared on disk (#2924). `entry` is
/// the raw declared JSON object (`command`/`args`/`env` or `url`/`headers`),
/// kept unresolved so `${CLAUDE_PLUGIN_ROOT}` expands per installed root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMcpServerSnapshot {
    pub name: String,
    pub entry: serde_json::Value,
}

/// Display metadata read from a plugin's `.claude-plugin/plugin.json`
/// (#2921), persisted with the source so the Skills view can show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

/// Serializable snapshot of validated hook configuration (for manifest storage).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HooksConfigSnapshot {
    pub version: Option<u32>,
    pub validated: bool,
}

#[derive(Debug, Clone)]
pub struct PluginSourceRecord {
    pub manifest: PluginSourceManifest,
    pub root_dir: PathBuf,
}
