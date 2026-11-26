use super::schema::{
    load_hooks_from_config_files, load_hooks_from_root, merge_hooks_inline, HookEvent, HookHandler,
    HooksConfig,
};
use crate::session::get_session_manager;
use log::warn;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const USER_PLUGINS_DIR_NAME: &str = "user_plugins";
/// Legacy location of per-session plugin stores
/// (`{data_dir}/session_plugins/{session_id}/`, #2825). #2942 moved the store
/// into the **default** session workspace directory (see
/// `SESSION_WORKSPACE_PLUGINS_DIR`); this constant now only drives cleanup of
/// leftover legacy stores on session deletion. Existing legacy entries are
/// ignored, not migrated (documented in the #2942 PR).
pub const LEGACY_SESSION_PLUGINS_DIR_NAME: &str = "session_plugins";
/// Hidden-directory name holding the session-scoped plugin store under the
/// **default** session workspace directory
/// (`{data_dir}/workspaces/<session_id>/.libragent/plugins`, #2942 D1
/// revised). Unlike tool-result spillover (which follows the current
/// workspace), the store never moves with a workspace override.
pub const SESSION_WORKSPACE_PLUGINS_DIR: &str = "plugins";
const MANIFEST_FILE: &str = "manifest.json";

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

fn enabled_by_default() -> bool {
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

pub fn get_user_plugins_directory() -> Result<PathBuf, String> {
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join(USER_PLUGINS_DIR_NAME))
}

/// Directory holding the plugins installed for one session only (#2825).
/// Per the revised owner decision D1 (#2942), the store lives under the
/// **default** session workspace directory
/// (`{data_dir}/workspaces/<session_id>/.libragent/plugins`), never under
/// the current (possibly overridden) workspace — a workspace change can
/// therefore never make session plugins disappear. Session deletion removes
/// the store together with the default directory
/// (`SessionDirectoryService::remove_workspace`).
/// Headless limit (#2942 G3.5): a headless run uses an ephemeral data dir
/// and its write-back copies DB rows only
/// (`headless/writeback.rs::write_back_*`), so plugins installed during an
/// ephemeral headless run are not written back to the real profile.
pub fn get_session_plugins_directory(session_id: &str) -> Result<PathBuf, String> {
    validate_session_id(session_id)?;
    let session_manager = get_session_manager()?;
    // #2942 G3.1: resolve from the session id only. The previous
    // `get_session_workspace_dir_by_id` returned the override once hydrated,
    // so the same session could resolve to two different stores.
    Ok(session_store_dir_in(
        &session_manager.get_session_default_workspace_dir_unverified(session_id),
    ))
}

/// Pure location of one session's plugin store
/// (`<default_workspace_root>/.libragent/plugins`), the join used by
/// [`get_session_plugins_directory`] (extracted for direct testing, #2942
/// gatekeeper G1.4). `default_workspace_root` must be the **default**
/// session workspace directory (`workspaces/<session_id>`), not the
/// current/override workspace (#2942 D1 revised).
pub fn session_store_dir_in(default_workspace_root: &Path) -> PathBuf {
    default_workspace_root
        .join(".libragent")
        .join(SESSION_WORKSPACE_PLUGINS_DIR)
}

fn validate_session_id(session_id: &str) -> Result<(), String> {
    if session_id.is_empty()
        || session_id == "."
        || session_id == ".."
        || session_id.contains(['/', '\\'])
    {
        return Err(format!("invalid session id: {:?}", session_id));
    }
    Ok(())
}

/// Directory name under the app data dir holding per-session plugin trust
/// records (#2942 gatekeeper G1.1). Plugin files and the store manifest live
/// in the session workspace (owner D1), but approval data never does: the
/// workspace is agent-writable, so a rewritten hook script plus a rewritten
/// workspace manifest must not be able to make a hook run.
pub const SESSION_PLUGIN_TRUST_DIR_NAME: &str = "session_plugin_trust";

/// `{data_dir}/session_plugin_trust/{session_id}` — the out-of-workspace
/// trust records for one session's plugins (see [`SessionPluginTrust`]).
pub fn get_session_plugin_trust_dir(session_id: &str) -> Result<PathBuf, String> {
    validate_session_id(session_id)?;
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join(SESSION_PLUGIN_TRUST_DIR_NAME)
        .join(session_id))
}

/// Directory name under the app data dir holding per-plugin data
/// (`${CLAUDE_PLUGIN_DATA}`, #2926). It lives in the app data dir, never in
/// the plugin store or the agent-writable workspace, so plugin data survives
/// both a plugin update and a reinstall of the same source (#2926 keeps it
/// when the plugin updates) and uninstall is the only thing that removes it.
pub const PLUGIN_DATA_DIR_NAME: &str = "plugin_data";

/// `{data_dir}/plugin_data/user/{source_id}` — `${CLAUDE_PLUGIN_DATA}` for a
/// globally installed plugin.
pub fn get_user_plugin_data_dir(source_id: &str) -> Result<PathBuf, String> {
    validate_source_id(source_id)?;
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join(PLUGIN_DATA_DIR_NAME)
        .join("user")
        .join(source_id))
}

/// `{data_dir}/plugin_data/sessions/{session_id}/{source_id}` —
/// `${CLAUDE_PLUGIN_DATA}` for a session-scope plugin. A session's plugin
/// data goes away with the plugin's uninstall in that session.
pub fn get_session_plugin_data_dir(session_id: &str, source_id: &str) -> Result<PathBuf, String> {
    validate_session_id(session_id)?;
    validate_source_id(source_id)?;
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join(PLUGIN_DATA_DIR_NAME)
        .join("sessions")
        .join(session_id)
        .join(source_id))
}

/// Create a plugin data directory (and its parents) and return it.
pub fn ensure_plugin_data_dir(dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir.to_path_buf())
}

/// `${CLAUDE_ENV_FILE}` for one session's `SessionStart` hooks (#2926): a
/// file the hook writes `NAME=value` lines to, which later shell tool calls
/// in that session then see. It lives in the app data dir, out of the agent
/// workspace, and is rewritten for every `SessionStart` run.
pub const PLUGIN_ENV_FILE_NAME: &str = "session-start.env";

pub fn get_session_plugin_env_file(session_id: &str) -> Result<PathBuf, String> {
    validate_session_id(session_id)?;
    match get_session_manager() {
        Ok(session_manager) => Ok(session_manager
            .get_base_data_dir()
            .join(PLUGIN_ENV_DIR_NAME)
            .join(session_id)
            .join(PLUGIN_ENV_FILE_NAME)),
        // No global session manager (unit tests, or a very early startup
        // state): keep the file out of the agent workspace all the same.
        Err(_) => Ok(std::env::temp_dir()
            .join("libragent-plugin-env")
            .join(session_id)
            .join(PLUGIN_ENV_FILE_NAME)),
    }
}

/// Directory name under the app data dir holding per-session
/// `CLAUDE_ENV_FILE` files.
pub const PLUGIN_ENV_DIR_NAME: &str = "plugin_env";

/// Approval state of one session-installed plugin, stored outside the
/// session workspace (see [`SESSION_PLUGIN_TRUST_DIR_NAME`]). This is the
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
/// [`list_sources_in`] on a session plugin directory.
///
/// Approval is overlaid from the out-of-workspace trust store
/// (`trust_dir`, see [`SESSION_PLUGIN_TRUST_DIR_NAME`]); the workspace
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

pub fn validate_source_id(source_id: &str) -> Result<(), String> {
    let valid = source_id.len() == 23
        && source_id.starts_with("plugin-")
        && source_id[7..]
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
    if !valid {
        return Err(format!("invalid plugin source_id: {:?}", source_id));
    }
    Ok(())
}

pub fn list_sources() -> Result<Vec<PluginSourceRecord>, String> {
    let base = get_user_plugins_directory()?;
    list_sources_in(&base)
}

pub fn list_sources_in(base: &Path) -> Result<Vec<PluginSourceRecord>, String> {
    if !base.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(base).map_err(|e| format!("read {}: {e}", base.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            if let Ok(record) = get_source_in(
                base,
                path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            ) {
                out.push(record);
            }
        }
    }
    out.sort_by(|a, b| a.manifest.source_id.cmp(&b.manifest.source_id));
    Ok(out)
}

pub fn get_source(source_id: &str) -> Result<PluginSourceRecord, String> {
    let base = get_user_plugins_directory()?;
    get_source_in(&base, source_id)
}

pub fn get_source_in(base: &Path, source_id: &str) -> Result<PluginSourceRecord, String> {
    validate_source_id(source_id)?;
    let root_dir = base.join(source_id);
    let manifest_path = root_dir.join(MANIFEST_FILE);
    if !manifest_path.is_file() {
        return Err(format!(
            "plugin source manifest not found: {}",
            manifest_path.display()
        ));
    }
    let raw = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("read {}: {e}", manifest_path.display()))?;
    let manifest: PluginSourceManifest = serde_json::from_str(&raw)
        .map_err(|e| format!("parse {}: {e}", manifest_path.display()))?;
    if manifest.source_id != source_id {
        return Err(format!(
            "manifest source_id {:?} does not match directory {:?}",
            manifest.source_id, source_id
        ));
    }
    Ok(PluginSourceRecord { manifest, root_dir })
}

pub fn save_source(manifest: PluginSourceManifest) -> Result<PluginSourceRecord, String> {
    let base = get_user_plugins_directory()?;
    save_source_in(&base, manifest)
}

pub fn save_source_in(
    base: &Path,
    manifest: PluginSourceManifest,
) -> Result<PluginSourceRecord, String> {
    validate_source_id(&manifest.source_id)?;
    let root_dir = base.join(&manifest.source_id);
    fs::create_dir_all(&root_dir).map_err(|e| format!("create {}: {e}", root_dir.display()))?;

    // Validate hooks on disk: exactly the persisted file list when present
    // (manifest-declared files included, gatekeeper G2.2), else the default
    // `hooks/` discovery for legacy stores. Inline manifest hooks (#3005) are
    // validated from the persisted JSON, not from disk.
    let hooks_snapshot = if !manifest.hook_files.is_empty() {
        let files: Vec<PathBuf> = manifest
            .hook_files
            .iter()
            .map(|rel| root_dir.join(rel))
            .collect();
        let cfg = load_hooks_from_config_files(&files)?;
        Some(HooksConfigSnapshot {
            version: cfg.version,
            validated: true,
        })
    } else if root_dir.join("hooks").is_dir() {
        let cfg = load_hooks_from_root(&root_dir)?;
        Some(HooksConfigSnapshot {
            version: cfg.version,
            validated: true,
        })
    } else if !manifest.inline_hooks.is_empty() {
        let mut cfg = HooksConfig::empty();
        merge_hooks_inline(
            &mut cfg,
            &manifest.inline_hooks,
            &format!("{} manifest hooks", manifest.source_id),
        )?;
        Some(HooksConfigSnapshot {
            version: cfg.version,
            validated: true,
        })
    } else {
        manifest.hooks.clone()
    };

    let to_store = PluginSourceManifest {
        hooks: hooks_snapshot,
        ..manifest
    };

    let manifest_path = root_dir.join(MANIFEST_FILE);
    let json =
        serde_json::to_string_pretty(&to_store).map_err(|e| format!("serialize manifest: {e}"))?;
    fs::write(&manifest_path, json)
        .map_err(|e| format!("write {}: {e}", manifest_path.display()))?;

    Ok(PluginSourceRecord {
        manifest: to_store,
        root_dir,
    })
}

/// Load a source's hook config exactly the way it was validated and approved
/// at install: the persisted `hook_files` when present, plus the inline hooks
/// the manifest declared (#3005), falling back to the default `hooks/`
/// discovery for stores recorded before the file list existed (gatekeeper
/// G2.2 — one loader for install, dispatch and overview).
pub fn hook_config_for_record(record: &PluginSourceRecord) -> Result<HooksConfig, String> {
    let mut cfg = if record.manifest.hook_files.is_empty() {
        if record.root_dir.join("hooks").is_dir() {
            load_hooks_from_root(&record.root_dir)?
        } else {
            HooksConfig::empty()
        }
    } else {
        let files: Vec<PathBuf> = record
            .manifest
            .hook_files
            .iter()
            .map(|rel| record.root_dir.join(rel))
            .collect();
        load_hooks_from_config_files(&files)?
    };
    merge_hooks_inline(
        &mut cfg,
        &record.manifest.inline_hooks,
        &format!("{} manifest hooks", record.manifest.source_id),
    )?;
    Ok(cfg)
}

pub fn delete_source(source_id: &str) -> Result<(), String> {
    let base = get_user_plugins_directory()?;
    delete_source_in(&base, source_id)
}

pub fn delete_source_in(base: &Path, source_id: &str) -> Result<(), String> {
    validate_source_id(source_id)?;
    let root_dir = base.join(source_id);
    if root_dir.is_dir() {
        fs::remove_dir_all(&root_dir).map_err(|e| format!("remove {}: {e}", root_dir.display()))?;
    }
    Ok(())
}

pub fn set_hooks_approved(
    source_id: &str,
    approved: bool,
    approval_hashes: BTreeMap<String, String>,
) -> Result<PluginSourceManifest, String> {
    let mut record = get_source(source_id)?;
    record.manifest.hooks_approved = approved;
    record.manifest.approval_hashes = approval_hashes;
    let saved = save_source(record.manifest)?;
    Ok(saved.manifest)
}

pub fn compute_script_hash(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{:x}", digest))
}

pub fn compute_handler_approval_hashes(
    plugin_root: &Path,
    config: &HooksConfig,
) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (event, groups) in &config.events {
        for group in groups {
            for handler in &group.handlers {
                let key = approval_hash_key(*event, group.matcher.as_deref(), handler);
                map.insert(key, approval_hash_value(plugin_root, handler));
            }
        }
    }
    map
}

pub fn approval_hash_key(event: HookEvent, matcher: Option<&str>, handler: &HookHandler) -> String {
    let args_suffix = handler
        .args
        .as_ref()
        .map(|args| format!("|args:{}", args.join("\u{1}")))
        .unwrap_or_default();
    let mut key = format!(
        "{}|{}|{}|{}{}",
        event.as_str(),
        matcher.unwrap_or("*"),
        handler.command,
        handler.powershell.as_deref().unwrap_or(""),
        args_suffix
    );
    // HTTP handlers carry no command, so the target URL distinguishes them
    // (#3009). Appended only when present, so keys of command handlers stay
    // byte-identical to earlier installs. Headers and allowed env-var names
    // are part of what the handler sends, so they must distinguish the key
    // too — otherwise two handlers that differ only by headers share one
    // key and one is skipped as "unapproved or modified" (#3041 G1.4).
    if let Some(http) = &handler.http {
        key.push_str("|url=");
        key.push_str(&http.url);
        for (name, value) in &http.headers {
            key.push_str("|header=");
            key.push_str(name);
            key.push('=');
            key.push_str(value);
        }
        for name in &http.allowed_env_vars {
            key.push_str("|env=");
            key.push_str(name);
        }
    }
    let digest = Sha256::digest(key.as_bytes());
    format!("{:x}", digest)
}

pub fn approval_hash_value(plugin_root: &Path, handler: &HookHandler) -> String {
    let mut hasher = Sha256::new();
    hasher.update(handler.command.as_bytes());
    hasher.update(b"\0");
    if let Some(ps) = &handler.powershell {
        hasher.update(ps.as_bytes());
    }
    hasher.update(b"\0");
    if let Some(shell) = &handler.shell {
        hasher.update(shell.as_bytes());
    }
    // The URL, header names, and allowed env-var names are part of what an
    // HTTP handler does, so they are part of what the user approved (#3009).
    if let Some(http) = &handler.http {
        hasher.update(b"\0url=");
        hasher.update(http.url.as_bytes());
        for (name, value) in &http.headers {
            hasher.update(b"\0header\0");
            hasher.update(name.as_bytes());
            hasher.update(b"=");
            hasher.update(value.as_bytes());
        }
        for var in &http.allowed_env_vars {
            hasher.update(b"\0env\0");
            hasher.update(var.as_bytes());
        }
    }
    // Exec-form arguments are part of what runs, so they are part of what the
    // user approved (#3005). Only appended when present, so a handler without
    // `args` keeps the hash an earlier install recorded.
    if let Some(args) = &handler.args {
        hasher.update(b"\0args\0");
        for arg in args {
            hasher.update(arg.as_bytes());
            hasher.update(b"\0");
        }
    }
    for (rel, file_hash) in collect_referenced_script_hashes(plugin_root, handler) {
        hasher.update(b"\0");
        hasher.update(rel.as_bytes());
        hasher.update(b"=");
        hasher.update(file_hash.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn collect_referenced_script_hashes(
    plugin_root: &Path,
    handler: &HookHandler,
) -> BTreeMap<String, String> {
    let mut hashes = BTreeMap::new();
    let canonical_root = plugin_root
        .canonicalize()
        .unwrap_or_else(|_| plugin_root.to_path_buf());
    let root_str = plugin_root.to_string_lossy();

    let expand = |text: &str| {
        text.replace("${LIBRAGENT_PLUGIN_ROOT}", root_str.as_ref())
            .replace("${CLAUDE_PLUGIN_ROOT}", root_str.as_ref())
            .replace("${CURSOR_PLUGIN_ROOT}", root_str.as_ref())
            .replace("$LIBRAGENT_PLUGIN_ROOT", root_str.as_ref())
            .replace("$CLAUDE_PLUGIN_ROOT", root_str.as_ref())
            .replace("$CURSOR_PLUGIN_ROOT", root_str.as_ref())
    };
    let record_candidate = |hashes: &mut BTreeMap<String, String>, tok: &str| {
        let candidate = if Path::new(tok).is_absolute() {
            PathBuf::from(tok)
        } else {
            plugin_root.join(tok)
        };
        if !candidate.is_file() {
            return;
        }
        let Ok(canonical_file) = candidate.canonicalize() else {
            return;
        };
        let Ok(rel) = canonical_file.strip_prefix(&canonical_root) else {
            return;
        };
        if let Ok(file_hash) = compute_script_hash(&canonical_file) {
            let rel_key = rel.to_string_lossy().replace('\\', "/");
            hashes.insert(rel_key, file_hash);
        }
    };

    let mut texts = vec![handler.command.as_str()];
    if let Some(ps) = handler.powershell.as_deref() {
        texts.push(ps);
    }

    for text in texts {
        let expanded = expand(text);
        for raw_tok in expanded.split_whitespace() {
            let tok = raw_tok.trim_matches(|c| matches!(c, '"' | '\'' | ';' | ','));
            if tok.is_empty() {
                continue;
            }
            record_candidate(&mut hashes, tok);
        }
    }

    // Exec form (`args` present, #3005): each argument is passed verbatim to
    // the spawned command, so a script path in argv runs exactly as written —
    // hash each element whole. Without this, `"command": "node",
    // "args": ["${CLAUDE_PLUGIN_ROOT}/x.js"]` would leave x.js unhashed and
    // editable after approval (gatekeeper G1.1). A whole argument is not
    // enough either: `"bash", ["-c", "./hooks/x.sh --fix"]` runs a script
    // wrapped inside a command string, so the path tokens inside each
    // argument are hashed too, exactly as for shell-form text (G2.2).
    if let Some(args) = handler.args.as_deref() {
        for arg in args {
            let expanded = expand(arg);
            if expanded.trim().is_empty() {
                continue;
            }
            record_candidate(&mut hashes, expanded.trim());
            for raw_tok in expanded.split_whitespace() {
                let tok = raw_tok.trim_matches(|c| matches!(c, '"' | '\'' | ';' | ','));
                if tok.is_empty() {
                    continue;
                }
                record_candidate(&mut hashes, tok);
            }
        }
    }
    hashes
}

pub fn list_approved_sources_in(base: &Path) -> Vec<PluginSourceRecord> {
    list_sources_in(base)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.manifest.hooks_approved && r.manifest.enabled)
        .collect()
}

/// Approved plugin sources effective for one session: the session's own
/// installs plus global installs that the session has not shadowed.
///
/// A session install wins over a global install with the same `source_id`
/// even when the session copy is not hook-approved, so a plugin's hooks never
/// run twice and a session can opt out of a global plugin's hooks (#2825).
pub fn list_effective_approved_sources_in(
    global_base: &Path,
    session_base: Option<&Path>,
    session_trust_dir: Option<&Path>,
) -> Vec<PluginSourceRecord> {
    let session_sources = match session_base {
        Some(base) => list_session_store_sources(session_trust_dir, base),
        None => Vec::new(),
    };
    let shadowed: HashSet<String> = session_sources
        .iter()
        .map(|r| r.manifest.source_id.clone())
        .collect();
    let mut out: Vec<PluginSourceRecord> = list_approved_sources_in(global_base)
        .into_iter()
        .filter(|r| !shadowed.contains(&r.manifest.source_id))
        .collect();
    out.extend(
        session_sources
            .into_iter()
            .filter(|r| r.manifest.hooks_approved && r.manifest.enabled),
    );
    out
}

pub fn list_approved_plugin_roots() -> Vec<PathBuf> {
    list_sources()
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.manifest.hooks_approved && r.manifest.enabled)
        .map(|r| r.root_dir)
        .collect()
}

/// Turn one installed global plugin on or off (#2927). The files stay in
/// place, so enabling again needs no download; loaders read
/// `manifest.enabled` and skip a disabled plugin. The global store lives in
/// the app data directory, outside every workspace, so the manifest is safe
/// to read as-is here (session plugins use
/// [`set_session_source_enabled_in`]). Returns the saved record.
pub fn set_source_enabled_in(
    base: &Path,
    source_id: &str,
    enabled: bool,
) -> Result<PluginSourceRecord, String> {
    let mut record = get_source_in(base, source_id)?;
    if record.manifest.enabled == enabled {
        return Ok(record);
    }
    record.manifest.enabled = enabled;
    save_source_in(base, record.manifest)
}

/// Turn one session plugin on or off (#2927). The state is written to the
/// out-of-workspace trust record, which is the only authority
/// ([`list_session_store_sources`] overlays it); the workspace manifest keeps
/// a copy only so a later reinstall still knows the state the user chose.
/// The returned record already carries the new state.
pub fn set_session_source_enabled_in(
    trust_dir: &Path,
    plugins_dir: &Path,
    source_id: &str,
    enabled: bool,
) -> Result<PluginSourceRecord, String> {
    let mut record = get_source_in(plugins_dir, source_id)?;
    let mut trust = read_session_plugin_trust(trust_dir, source_id);
    if trust.enabled != enabled {
        trust.enabled = enabled;
        record_session_plugin_trust(trust_dir, source_id, &trust)?;
    }
    record.manifest.enabled = enabled;
    // Best effort: a stale copy only affects the state a reinstall starts
    // from, never what the current session loads, so it must not fail a
    // request the user can see succeed.
    if let Err(error) = save_source_in(plugins_dir, record.manifest.clone()) {
        warn!("Could not refresh the reinstall state of session plugin {source_id}: {error}");
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #2921 (gatekeeper G1.2): `.claude-plugin/plugin.json` metadata must
    /// survive `save_source_in`/`get_source_in` so the Skills view can show
    /// plugin name/version/description across restarts.
    #[test]
    fn plugin_metadata_roundtrips_through_source_store() {
        let base = tempfile::tempdir().unwrap();
        let source_id = format!("plugin-{}", "0".repeat(16));
        // The persisted hook_files list is validated against disk at save,
        // so lay down the file it names.
        let source_root = base.path().join(&source_id);
        fs::create_dir_all(source_root.join("hooks")).unwrap();
        fs::write(
            source_root.join("hooks").join("hooks.json"),
            r#"{"hooks":{"SessionStart":[{"command":"echo hi"}]}}"#,
        )
        .unwrap();
        let record = save_source_in(
            base.path(),
            PluginSourceManifest {
                mcp_servers: Vec::new(),
                mcp_approved: false,
                source_url: "https://example.com/p".to_string(),
                source_id: source_id.clone(),
                skill_names: vec!["alpha".to_string()],
                hooks: None,
                hooks_approved: false,
                approval_hashes: BTreeMap::new(),
                created_at: 0,
                hook_files: vec!["./hooks/hooks.json".to_string()],
                inline_hooks: vec![],
                plugin: Some(PluginMetadata {
                    name: "superpowers".to_string(),
                    version: Some("1.2.3".to_string()),
                    description: Some("Skills for agents".to_string()),
                    author: Some("Jesse Vincent".to_string()),
                    homepage: None,
                    repository: None,
                    license: None,
                    keywords: vec!["skills".to_string()],
                }),
                enabled: true,
                installed_commit: None,
            },
        )
        .unwrap();
        assert!(record.manifest.plugin.is_some());

        let loaded = get_source_in(base.path(), &source_id).unwrap();
        let plugin = loaded
            .manifest
            .plugin
            .as_ref()
            .expect("plugin metadata persisted");
        assert_eq!(plugin.name, "superpowers");
        assert_eq!(plugin.version.as_deref(), Some("1.2.3"));
        assert_eq!(plugin.description.as_deref(), Some("Skills for agents"));
        assert_eq!(plugin.author.as_deref(), Some("Jesse Vincent"));
        assert_eq!(plugin.keywords, vec!["skills".to_string()]);
        assert_eq!(loaded.manifest.skill_names, vec!["alpha".to_string()]);
        assert_eq!(
            loaded.manifest.hook_files,
            vec!["./hooks/hooks.json".to_string()],
        );

        let cfg = hook_config_for_record(&loaded).unwrap();
        assert!(cfg.events.contains_key(&HookEvent::SessionStart));
    }

    /// Inline manifest hooks (#3005): a plugin with no `hooks/` directory still
    /// dispatches the hooks its manifest declares, and the stored JSON is what
    /// dispatch reads.
    #[test]
    fn inline_manifest_hooks_load_for_a_plugin_without_a_hooks_dir() {
        let base = tempfile::tempdir().unwrap();
        let source_id = format!("plugin-{}", "2".repeat(16));
        let root_dir = base.path().join(&source_id);
        fs::create_dir_all(&root_dir).unwrap();
        let manifest = PluginSourceManifest {
            source_url: "test://local".to_string(),
            source_id: source_id.clone(),
            skill_names: vec![],
            hooks: None,
            hooks_approved: true,
            approval_hashes: BTreeMap::new(),
            created_at: 0,
            plugin: None,
            hook_files: vec![],
            inline_hooks: vec![serde_json::json!({
                "PreToolUse": [{ "matcher": "Bash", "command": "echo inline" }],
                "SessionEnd": [{ "command": "echo bye" }]
            })],
            mcp_servers: Vec::new(),
            mcp_approved: false,
            enabled: true,
            installed_commit: None,
        };
        let record = save_source_in(base.path(), manifest).unwrap();
        assert!(
            record.manifest.hooks.as_ref().is_some_and(|h| h.validated),
            "inline hooks must be validated at save time"
        );

        let cfg = hook_config_for_record(&record).unwrap();
        assert_eq!(
            cfg.events[&HookEvent::PreToolUse][0].handlers[0].command,
            "echo inline"
        );
        assert!(cfg.events.contains_key(&HookEvent::SessionEnd));
    }

    /// A record whose inline hooks are invalid fails the save instead of
    /// installing a plugin that would dispatch nothing (#3005).
    #[test]
    fn invalid_inline_manifest_hooks_fail_the_save() {
        let base = tempfile::tempdir().unwrap();
        let source_id = format!("plugin-{}", "3".repeat(16));
        fs::create_dir_all(base.path().join(&source_id)).unwrap();
        let manifest = PluginSourceManifest {
            source_url: "test://local".to_string(),
            source_id,
            skill_names: vec![],
            hooks: None,
            hooks_approved: true,
            approval_hashes: BTreeMap::new(),
            created_at: 0,
            plugin: None,
            hook_files: vec![],
            inline_hooks: vec![serde_json::json!({ "PreToolUse": "not-an-array" })],
            mcp_servers: Vec::new(),
            mcp_approved: false,
            enabled: true,
            installed_commit: None,
        };
        let error = save_source_in(base.path(), manifest).unwrap_err();
        assert!(error.contains("PreToolUse"), "{error}");
    }

    /// #2927: a disabled plugin keeps its files and approval record, but the
    /// hook loaders skip it, and the flag round-trips through the store.
    #[test]
    fn disabled_source_is_skipped_by_hook_loaders() {
        let base = tempfile::tempdir().unwrap();
        let source_id = format!("plugin-{}", "1".repeat(16));
        save_source_in(base.path(), test_manifest(&source_id, true)).unwrap();
        assert_eq!(list_approved_sources_in(base.path()).len(), 1);

        let disabled = set_source_enabled_in(base.path(), &source_id, false).unwrap();
        assert!(!disabled.manifest.enabled);
        assert!(
            list_approved_sources_in(base.path()).is_empty(),
            "a disabled plugin must contribute no hooks"
        );

        let still_stored = get_source_in(base.path(), &source_id).unwrap();
        assert!(!still_stored.manifest.enabled);
        assert!(
            still_stored.manifest.hooks_approved,
            "disabling keeps the one-time approval record"
        );
        assert!(
            still_stored.root_dir.is_dir(),
            "disabling keeps the installed files"
        );

        let enabled_again = set_source_enabled_in(base.path(), &source_id, true).unwrap();
        assert!(enabled_again.manifest.enabled);
        assert_eq!(list_approved_sources_in(base.path()).len(), 1);
    }

    /// #2927: a store recorded before the flag existed has no `enabled` key
    /// and must load enabled, so an upgrade never silently disables a plugin.
    #[test]
    fn legacy_manifest_without_enabled_field_loads_enabled() {
        let base = tempfile::tempdir().unwrap();
        let source_id = format!("plugin-{}", "2".repeat(16));
        let root = base.path().join(&source_id);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(MANIFEST_FILE),
            format!(
                r#"{{"source_url":"https://example.com/p","source_id":"{source_id}",
                    "skill_names":["alpha"],"hooks":null,"hooks_approved":true,
                    "approval_hashes":{{}},"created_at":0}}"#
            ),
        )
        .unwrap();

        let record = get_source_in(base.path(), &source_id).unwrap();
        assert!(
            record.manifest.enabled,
            "pre-#2927 stores must load enabled"
        );
        assert_eq!(record.manifest.installed_commit, None);
        assert_eq!(list_approved_sources_in(base.path()).len(), 1);
    }

    /// #2942 gatekeeper G1.4 fixtures: a manifest with optional inline
    /// approval, without touching the real install pipeline. `mcp_approved`
    /// mirrors the D3 install gate (approval covers MCP servers too,
    /// #2942 S3.2).
    fn test_manifest(source_id: &str, approved: bool) -> PluginSourceManifest {
        PluginSourceManifest {
            source_url: "https://example.com/p".to_string(),
            source_id: source_id.to_string(),
            skill_names: vec![],
            hooks: None,
            hooks_approved: approved,
            approval_hashes: if approved {
                BTreeMap::from([("SessionStart:0".to_string(), "deadbeef".to_string())])
            } else {
                BTreeMap::new()
            },
            created_at: 0,
            plugin: None,
            hook_files: vec![],
            inline_hooks: vec![],
            mcp_servers: if approved {
                vec![PluginMcpServerSnapshot {
                    name: "srv".to_string(),
                    entry: serde_json::json!({ "command": "bin/srv" }),
                }]
            } else {
                vec![]
            },
            mcp_approved: approved,
            enabled: true,
            installed_commit: None,
        }
    }

    fn entry(source_id: &str) -> SessionPluginStoreEntry {
        SessionPluginStoreEntry {
            source_id: source_id.to_string(),
            source_url: "https://example.com/p".to_string(),
            installed_at: 0,
        }
    }

    #[test]
    fn session_store_dir_is_under_workspace_and_isolated_per_session() {
        // #2942 gatekeeper G1.4: the store lives inside the session
        // workspace at `.libragent/plugins`, and two workspaces never share
        // a store.
        let ws_a = tempfile::tempdir().unwrap();
        let ws_b = tempfile::tempdir().unwrap();
        let store_a = session_store_dir_in(ws_a.path());
        assert_eq!(
            store_a,
            ws_a.path()
                .join(".libragent")
                .join(SESSION_WORKSPACE_PLUGINS_DIR)
        );
        let store_b = session_store_dir_in(ws_b.path());
        assert!(!store_a.starts_with(&store_b) && !store_b.starts_with(&store_a));

        let id = format!("plugin-{}", "a".repeat(16));
        save_source_in(&store_a, test_manifest(&id, false)).unwrap();
        upsert_session_store_entry(&store_a, entry(&id)).unwrap();
        assert_eq!(list_session_store_sources(None, &store_a).len(), 1);
        assert!(list_session_store_sources(None, &store_b).is_empty());
    }

    #[test]
    fn move_session_approval_to_trust_roundtrips_and_overlays() {
        // #2942 gatekeeper G1.1: after the move, the workspace manifest
        // holds no approval and only the trust record can approve; loaders
        // without the trust dir (or with a missing one) stay unapproved.
        let base = tempfile::tempdir().unwrap();
        let trust_root = tempfile::tempdir().unwrap();
        let trust_dir = trust_root.path().join("s1");
        let id = format!("plugin-{}", "b".repeat(16));
        save_source_in(base.path(), test_manifest(&id, true)).unwrap();
        upsert_session_store_entry(base.path(), entry(&id)).unwrap();

        let before = get_source_in(base.path(), &id).unwrap();
        assert!(before.manifest.hooks_approved);
        assert!(before.manifest.mcp_approved);

        move_session_approval_to_trust(&trust_dir, base.path(), &id).unwrap();

        let scrubbed = get_source_in(base.path(), &id).unwrap();
        assert!(!scrubbed.manifest.hooks_approved);
        assert!(scrubbed.manifest.approval_hashes.is_empty());
        assert!(!scrubbed.manifest.mcp_approved);
        let trust_record = read_session_plugin_trust(&trust_dir, &id);
        assert!(trust_record.hooks_approved);
        assert!(trust_record.mcp_approved);
        assert_eq!(
            trust_record.approval_hashes,
            before.manifest.approval_hashes
        );

        let with_trust = list_session_store_sources(Some(&trust_dir), base.path());
        assert_eq!(with_trust.len(), 1);
        assert!(with_trust[0].manifest.hooks_approved);
        assert!(with_trust[0].manifest.mcp_approved);
        assert_eq!(
            with_trust[0].manifest.approval_hashes,
            before.manifest.approval_hashes
        );
        assert!(
            !list_session_store_sources(None, base.path())[0]
                .manifest
                .hooks_approved
        );
        assert!(
            !list_session_store_sources(None, base.path())[0]
                .manifest
                .mcp_approved
        );
        let other_trust = trust_root.path().join("s2");
        assert!(
            !list_session_store_sources(Some(&other_trust), base.path())[0]
                .manifest
                .hooks_approved
        );
        assert!(
            !list_session_store_sources(Some(&other_trust), base.path())[0]
                .manifest
                .mcp_approved
        );

        // Uninstall removes the trust record; a missing file reads as
        // unapproved (fail-safe).
        remove_session_plugin_trust(&trust_dir, &id).unwrap();
        assert!(!read_session_plugin_trust(&trust_dir, &id).hooks_approved);
        assert!(!read_session_plugin_trust(&trust_dir, &id).mcp_approved);
        remove_session_plugin_trust(&trust_dir, &id).unwrap();
    }

    #[test]
    fn tampered_workspace_mcp_approved_flag_is_ignored() {
        // #2942 S3.2: the session store now lives in the agent-writable
        // workspace, so flipping mcp_approved in the workspace manifest
        // (without touching {data_dir}) must never approve a server.
        let base = tempfile::tempdir().unwrap();
        let trust_dir = tempfile::tempdir().unwrap().path().join("s1");
        let id = format!("plugin-{}", "c".repeat(16));
        save_source_in(base.path(), test_manifest(&id, false)).unwrap();
        upsert_session_store_entry(base.path(), entry(&id)).unwrap();

        // Tamper: rewrite the workspace manifest as if the user had approved.
        let manifest_path = base.path().join(&id).join(MANIFEST_FILE);
        let raw = fs::read_to_string(&manifest_path).unwrap();
        let mut json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        json["mcp_approved"] = serde_json::Value::Bool(true);
        fs::write(&manifest_path, serde_json::to_string_pretty(&json).unwrap()).unwrap();

        // Load with the (empty) trust dir and without one: stays unapproved.
        let records = list_session_store_sources(Some(&trust_dir), base.path());
        assert_eq!(records.len(), 1);
        assert!(!records[0].manifest.mcp_approved);
        assert!(
            !list_session_store_sources(None, base.path())[0]
                .manifest
                .mcp_approved
        );
    }

    #[test]
    fn tampered_workspace_enabled_flag_is_ignored() {
        // #2927 gatekeeper G1.1: the enabled state of a session plugin lives
        // in the trust record, so a workspace manifest edited to
        // `"enabled": true` must not turn a plugin the user disabled back on.
        let base = tempfile::tempdir().unwrap();
        let trust_dir = tempfile::tempdir().unwrap().path().join("s1");
        let id = format!("plugin-{}", "d".repeat(16));
        save_source_in(base.path(), test_manifest(&id, true)).unwrap();
        upsert_session_store_entry(base.path(), entry(&id)).unwrap();
        move_session_approval_to_trust(&trust_dir, base.path(), &id).unwrap();

        let disabled = set_session_source_enabled_in(&trust_dir, base.path(), &id, false).unwrap();
        assert!(!disabled.manifest.enabled);
        assert!(
            !read_session_plugin_trust(&trust_dir, &id).enabled,
            "the trust record is where the state is kept"
        );

        // Tamper: rewrite the workspace manifest as if the user had enabled
        // it again.
        let manifest_path = base.path().join(&id).join(MANIFEST_FILE);
        let raw = fs::read_to_string(&manifest_path).unwrap();
        let mut json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        json["enabled"] = serde_json::Value::Bool(true);
        fs::write(&manifest_path, serde_json::to_string_pretty(&json).unwrap()).unwrap();

        let records = list_session_store_sources(Some(&trust_dir), base.path());
        assert_eq!(records.len(), 1);
        assert!(
            !records[0].manifest.enabled,
            "a workspace edit must not re-enable a disabled plugin"
        );
        assert!(
            records[0].manifest.hooks_approved,
            "the approval overlay still comes from the trust record"
        );
        // ... so its approved hooks still do not run.
        let no_global = tempfile::tempdir().unwrap();
        assert!(
            list_effective_approved_sources_in(
                no_global.path(),
                Some(base.path()),
                Some(&trust_dir)
            )
            .is_empty(),
            "a disabled plugin contributes no hooks"
        );

        // Only the trust record enables it again.
        set_session_source_enabled_in(&trust_dir, base.path(), &id, true).unwrap();
        assert!(
            list_session_store_sources(Some(&trust_dir), base.path())[0]
                .manifest
                .enabled
        );
    }

    #[test]
    fn session_plugin_without_trust_record_stays_enabled() {
        // #2927: a plugin installed before the trust record carried `enabled`
        // (or with no trust store at all) must load enabled — the state it
        // had before this field existed — while remaining unapproved.
        let base = tempfile::tempdir().unwrap();
        let trust_root = tempfile::tempdir().unwrap();
        let id = format!("plugin-{}", "e".repeat(16));
        save_source_in(base.path(), test_manifest(&id, true)).unwrap();
        upsert_session_store_entry(base.path(), entry(&id)).unwrap();

        let record = list_session_store_sources(None, base.path());
        assert_eq!(record.len(), 1);
        assert!(record[0].manifest.enabled);
        assert!(!record[0].manifest.hooks_approved);

        let empty_trust = trust_root.path().join("s1");
        assert!(
            list_session_store_sources(Some(&empty_trust), base.path())[0]
                .manifest
                .enabled,
            "a missing trust file must not disable a plugin"
        );
    }

    fn exec_handler(command: &str, args: Option<Vec<String>>) -> HookHandler {
        HookHandler {
            command: command.to_string(),
            args,
            powershell: None,
            timeout_ms: crate::services::hooks::schema::default_hook_timeout_ms(
                HookEvent::PreToolUse,
            ),
            r#async: false,
            shell: None,
            http: None,
        }
    }

    /// #3005 gatekeeper G1.1: in exec form the script usually lives in
    /// `args`, so its content must be part of the approval hash — editing it
    /// after approval invalidates the handler.
    #[test]
    fn exec_form_args_script_edit_invalidates_the_approval_hash() {
        let root = tempfile::tempdir().unwrap();
        let script = root.path().join("x.js");
        fs::write(&script, "console.log('v1')").unwrap();
        let handler = exec_handler("node", Some(vec!["${CLAUDE_PLUGIN_ROOT}/x.js".to_string()]));

        let before = approval_hash_value(root.path(), &handler);
        fs::write(&script, "console.log('evil')").unwrap();
        let after = approval_hash_value(root.path(), &handler);

        assert_ne!(
            before, after,
            "editing a script referenced by exec-form args must change the approval hash"
        );
    }

    /// #3005 gatekeeper G2.2: an exec-form argument can wrap a whole command
    /// string (`bash -c "./hooks/x.sh --fix"`), where no argument is a file
    /// path on its own. The path tokens inside each argument must be hashed
    /// too, or x.sh can be edited after approval and still run.
    #[test]
    fn exec_form_command_string_path_tokens_are_hashed() {
        let root = tempfile::tempdir().unwrap();
        let hooks = root.path().join("hooks");
        fs::create_dir_all(&hooks).unwrap();
        let script = hooks.join("x.sh");
        fs::write(&script, "#!/bin/sh\necho v1").unwrap();
        let handler = exec_handler(
            "bash",
            Some(vec!["-c".to_string(), "./hooks/x.sh --fix".to_string()]),
        );

        let before = approval_hash_value(root.path(), &handler);
        fs::write(&script, "#!/bin/sh\necho evil").unwrap();
        let after = approval_hash_value(root.path(), &handler);

        assert_ne!(
            before, after,
            "editing a script wrapped inside an exec-form argument must change the approval hash"
        );
    }

    /// #3005 gatekeeper G1.3: two handlers with the same event, matcher and
    /// command but different `args` get different approval keys, while a
    /// handler without `args` keeps the key earlier installs recorded.
    #[test]
    fn approval_hash_key_separates_handlers_by_args() {
        let first = exec_handler("node", Some(vec!["a.js".to_string()]));
        let second = exec_handler("node", Some(vec!["b.js".to_string()]));
        let plain = exec_handler("node", None);

        assert_ne!(
            approval_hash_key(HookEvent::PreToolUse, None, &first),
            approval_hash_key(HookEvent::PreToolUse, None, &second),
            "same command with different argv must not share one approval key"
        );

        // Keys of handlers without `args` are unchanged: the digest input of
        // the pre-#3005 formula.
        let expected_legacy = format!(
            "{:x}",
            Sha256::digest(
                format!(
                    "{}|*|{}|{}",
                    HookEvent::PreToolUse.as_str(),
                    plain.command,
                    ""
                )
                .as_bytes(),
            )
        );
        assert_eq!(
            approval_hash_key(HookEvent::PreToolUse, None, &plain),
            expected_legacy,
            "a handler without args must keep the key an earlier install recorded"
        );
    }
}
