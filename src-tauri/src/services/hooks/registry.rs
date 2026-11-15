use super::schema::load_hooks_from_root;
use crate::session::get_session_manager;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const USER_PLUGINS_DIR_NAME: &str = "user_plugins";
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

pub fn list_sources() -> Result<Vec<PluginSourceRecord>, String> {
    let base = get_user_plugins_directory()?;
    if !base.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&base).map_err(|e| format!("read {}: {e}", base.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            if let Ok(record) = get_source(path.file_name().and_then(|n| n.to_str()).unwrap_or(""))
            {
                out.push(record);
            }
        }
    }
    out.sort_by(|a, b| a.manifest.source_id.cmp(&b.manifest.source_id));
    Ok(out)
}

pub fn get_source(source_id: &str) -> Result<PluginSourceRecord, String> {
    if source_id.is_empty() {
        return Err("source_id is required".to_string());
    }
    let root_dir = get_user_plugins_directory()?.join(source_id);
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
    let root_dir = get_user_plugins_directory()?.join(&manifest.source_id);
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
    let root_dir = get_user_plugins_directory()?.join(source_id);
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

pub fn list_approved_plugin_roots() -> Vec<PathBuf> {
    list_sources()
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.manifest.hooks_approved)
        .map(|r| r.root_dir)
        .collect()
}
