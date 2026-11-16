use super::schema::{load_hooks_from_root, HookEvent, HookHandler, HooksConfig};
use crate::session::get_session_manager;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const USER_PLUGINS_DIR_NAME: &str = "user_plugins";
/// Per-session plugin stores live at `{data_dir}/session_plugins/{session_id}/`.
/// They are deliberately kept outside the agent-writable workspace so the agent
/// cannot rewrite a hook script together with its approval manifest. Session
/// deletion removes the store (`SessionDirectoryService::remove_workspace`).
pub const SESSION_PLUGINS_DIR_NAME: &str = "session_plugins";
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
///
/// Anchored at the app data dir (see [`SESSION_PLUGINS_DIR_NAME`]).
pub fn get_session_plugins_directory(session_id: &str) -> Result<PathBuf, String> {
    if session_id.is_empty()
        || session_id == "."
        || session_id == ".."
        || session_id.contains(['/', '\\'])
    {
        return Err(format!("invalid session id: {:?}", session_id));
    }
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join(SESSION_PLUGINS_DIR_NAME)
        .join(session_id))
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

    // Validate hooks on disk when present.
    let hooks_snapshot = if root_dir.join("hooks").is_dir() {
        let cfg = load_hooks_from_root(&root_dir)?;
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
    let digest = Sha256::digest(
        format!(
            "{}|{}|{}|{}",
            event.as_str(),
            matcher.unwrap_or("*"),
            handler.command,
            handler.powershell.as_deref().unwrap_or("")
        )
        .as_bytes(),
    );
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

    let mut texts = vec![handler.command.as_str()];
    if let Some(ps) = handler.powershell.as_deref() {
        texts.push(ps);
    }

    for text in texts {
        let expanded = text
            .replace("${LIBRAGENT_PLUGIN_ROOT}", root_str.as_ref())
            .replace("${CLAUDE_PLUGIN_ROOT}", root_str.as_ref())
            .replace("${CURSOR_PLUGIN_ROOT}", root_str.as_ref())
            .replace("$LIBRAGENT_PLUGIN_ROOT", root_str.as_ref())
            .replace("$CLAUDE_PLUGIN_ROOT", root_str.as_ref())
            .replace("$CURSOR_PLUGIN_ROOT", root_str.as_ref());

        for raw_tok in expanded.split_whitespace() {
            let tok = raw_tok.trim_matches(|c| matches!(c, '"' | '\'' | ';' | ','));
            if tok.is_empty() {
                continue;
            }
            let candidate = if Path::new(tok).is_absolute() {
                PathBuf::from(tok)
            } else {
                plugin_root.join(tok)
            };
            if !candidate.is_file() {
                continue;
            }
            let Ok(canonical_file) = candidate.canonicalize() else {
                continue;
            };
            let Ok(rel) = canonical_file.strip_prefix(&canonical_root) else {
                continue;
            };
            if let Ok(file_hash) = compute_script_hash(&canonical_file) {
                let rel_key = rel.to_string_lossy().replace('\\', "/");
                hashes.insert(rel_key, file_hash);
            }
        }
    }
    hashes
}

pub fn list_approved_sources_in(base: &Path) -> Vec<PluginSourceRecord> {
    list_sources_in(base)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.manifest.hooks_approved)
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
) -> Vec<PluginSourceRecord> {
    let session_sources = session_base
        .map(|base| list_sources_in(base).unwrap_or_default())
        .unwrap_or_default();
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
            .filter(|r| r.manifest.hooks_approved),
    );
    out
}

pub fn list_approved_plugin_roots() -> Vec<PathBuf> {
    list_sources()
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.manifest.hooks_approved)
        .map(|r| r.root_dir)
        .collect()
}
