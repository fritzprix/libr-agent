use super::super::schema::{
    load_hooks_from_config_files, load_hooks_from_root, merge_hooks_inline, HooksConfig,
};
use super::dirs::get_user_plugins_directory;
use super::trust::{read_session_plugin_trust, record_session_plugin_trust};
use super::{HooksConfigSnapshot, PluginSourceManifest, PluginSourceRecord, MANIFEST_FILE};
use log::warn;
use std::fs;
use std::path::{Path, PathBuf};

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
/// ([`super::list_session_store_sources`] overlays it); the workspace manifest keeps
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
