use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use log::warn;
use serde::{Deserialize, Serialize};

use super::discover::browser_default_priority;
use crate::session::get_session_manager;

const REGISTRY_FILE: &str = "browser_profiles.json";
const PROFILES_DIR: &str = "browser_profiles";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ImportKind {
    #[default]
    ChromiumUserData,
    FirefoxCookies,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedProfile {
    pub label: String,
    /// Absolute path to the LibrAgent-owned User Data root (not shown to agents).
    pub user_data_dir: PathBuf,
    pub source_label: String,
    pub source_browser: String,
    /// How the profile should be applied at session create time.
    #[serde(default)]
    pub import_kind: ImportKind,
    pub imported_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfileRegistry {
    /// Registry key used when `use_profile: true`.
    #[serde(default)]
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, ImportedProfile>,
    /// Legacy field retained for older registry files; unused (import is Settings-only).
    #[serde(default)]
    #[allow(dead_code)]
    pub first_import_attempted: bool,
}

/// Public profile status for Settings / Tauri (no raw source paths).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfileInfo {
    pub name: String,
    pub label: String,
    pub source_label: String,
    pub source_browser: String,
    pub imported_at: DateTime<Utc>,
    pub is_default: bool,
}

pub fn profiles_storage_root() -> Result<PathBuf, String> {
    // Prefer SessionManager when the main process has initialized it; fall back to
    // the same dirs::data_dir layout so the browser sidecar can validate paths too.
    if let Ok(manager) = get_session_manager() {
        return Ok(manager.get_base_data_dir().join(PROFILES_DIR));
    }
    Ok(dirs::data_dir()
        .ok_or_else(|| "Failed to resolve system data directory".to_string())?
        .join("com.fritzprix.libragent")
        .join(PROFILES_DIR))
}

pub fn registry_path() -> Result<PathBuf, String> {
    Ok(profiles_storage_root()?
        .parent()
        .ok_or_else(|| "Invalid browser profiles storage root".to_string())?
        .join(REGISTRY_FILE))
}

pub fn load_registry() -> Result<BrowserProfileRegistry, String> {
    let path = registry_path()?;
    if !path.exists() {
        return Ok(BrowserProfileRegistry::default());
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read browser profile registry: {e}"))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse browser profile registry: {e}"))
}

pub fn save_registry(registry: &BrowserProfileRegistry) -> Result<(), String> {
    let path = registry_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create app data directory: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(registry)
        .map_err(|e| format!("Failed to serialize browser profile registry: {e}"))?;
    std::fs::write(&path, raw)
        .map_err(|e| format!("Failed to write browser profile registry: {e}"))
}

pub fn has_any_imported_profile() -> Result<bool, String> {
    Ok(!load_registry()?.profiles.is_empty())
}

pub fn list_imported_profiles() -> Result<Vec<BrowserProfileInfo>, String> {
    let registry = load_registry()?;
    let default_name = registry
        .default_profile
        .clone()
        .or_else(|| preferred_profile_name(&registry));

    Ok(registry
        .profiles
        .iter()
        .map(|(name, profile)| BrowserProfileInfo {
            name: name.clone(),
            label: profile.label.clone(),
            source_label: profile.source_label.clone(),
            source_browser: profile.source_browser.clone(),
            imported_at: profile.imported_at,
            is_default: default_name.as_ref() == Some(name),
        })
        .collect())
}

/// Pick the best remaining profile using the same priority as import (Chrome > Edge > …).
fn preferred_profile_name(registry: &BrowserProfileRegistry) -> Option<String> {
    registry
        .profiles
        .iter()
        .min_by_key(|(name, profile)| {
            (
                browser_default_priority(&profile.source_browser),
                (*name).clone(),
            )
        })
        .map(|(name, _)| name.clone())
}

/// Ensure `path` resolves under the app-local browser_profiles storage root.
pub fn ensure_under_profiles_storage(path: &Path) -> Result<PathBuf, String> {
    let root = profiles_storage_root()?;
    std::fs::create_dir_all(&root)
        .map_err(|e| format!("Failed to create browser profiles storage directory: {e}"))?;

    let canonical_root = root.canonicalize().map_err(|e| {
        format!(
            "Failed to resolve browser profiles storage root {}: {e}",
            root.display()
        )
    })?;

    if !path.exists() {
        let normalized = normalize_path_components(path);
        if !normalized.starts_with(&root) && !normalized.starts_with(&canonical_root) {
            return Err(
                "Refusing to use a browser profile path outside LibrAgent app storage.".to_string(),
            );
        }
        return Ok(normalized);
    }

    let canonical_path = path.canonicalize().map_err(|e| {
        format!(
            "Failed to resolve browser profile path {}: {e}",
            path.display()
        )
    })?;

    if !canonical_path.starts_with(&canonical_root) {
        warn!(
            "Rejected browser profile path outside storage root: {}",
            path.display()
        );
        return Err(
            "Refusing to use a browser profile path outside LibrAgent app storage.".to_string(),
        );
    }

    Ok(canonical_path)
}

fn normalize_path_components(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                let _ = out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Resolve the app-local User Data directory for the default imported profile.
pub fn resolve_default_imported_user_data_dir() -> Result<PathBuf, String> {
    let registry = load_registry()?;
    let name = registry
        .default_profile
        .clone()
        .or_else(|| preferred_profile_name(&registry))
        .ok_or_else(|| {
            "No imported browser profile found. Import a browser profile from Settings first."
                .to_string()
        })?;

    let profile = registry.profiles.get(&name).ok_or_else(|| {
        format!("Default browser profile '{name}' is missing from the registry. Re-import from Settings.")
    })?;

    let user_data_dir = ensure_under_profiles_storage(&profile.user_data_dir)?;

    if !user_data_dir.is_dir() {
        return Err(
            "Imported browser profile directory is missing. Re-import from Settings.".to_string(),
        );
    }

    Ok(user_data_dir)
}

pub fn imported_profile_user_data_dir(profile_name: &str) -> Result<PathBuf, String> {
    if profile_name.is_empty()
        || profile_name.contains(['/', '\\'])
        || profile_name.contains("..")
    {
        return Err("Invalid browser profile name.".to_string());
    }
    let path = profiles_storage_root()?.join(profile_name);
    ensure_under_profiles_storage(&path)
}

pub fn remove_imported_profile(name: &str) -> Result<(), String> {
    let mut registry = load_registry()?;
    let Some(profile) = registry.profiles.remove(name) else {
        return Err(format!("Imported browser profile '{name}' was not found."));
    };

    if registry.default_profile.as_deref() == Some(name) {
        registry.default_profile = preferred_profile_name(&registry);
    }
    save_registry(&registry)?;

    if profile.user_data_dir.exists() {
        let safe_path = ensure_under_profiles_storage(&profile.user_data_dir)?;
        std::fs::remove_dir_all(&safe_path).map_err(|e| {
            format!(
                "Removed profile '{name}' from the registry, but failed to delete local files: {e}"
            )
        })?;
    }

    Ok(())
}

/// Mark an imported profile as the one agents use with `use_profile: true`.
pub fn set_default_imported_profile(name: &str) -> Result<(), String> {
    let mut registry = load_registry()?;
    if !registry.profiles.contains_key(name) {
        return Err(format!("Imported browser profile '{name}' was not found."));
    }
    registry.default_profile = Some(name.to_string());
    save_registry(&registry)
}

pub fn upsert_imported_profile(
    registry: &mut BrowserProfileRegistry,
    name: String,
    profile: ImportedProfile,
    set_as_default: bool,
) {
    if set_as_default || registry.default_profile.is_none() {
        registry.default_profile = Some(name.clone());
    }
    registry.profiles.insert(name, profile);
}
