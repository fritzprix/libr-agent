//! Plugin `userConfig` values (#2926).
//!
//! A plugin asks for values in its manifest `userConfig` map. The user enters
//! them at install. A value marked `sensitive` is stored in the app's secure
//! store (`crate::mcp::secret_templates::secret_store` — the OS keychain with
//! the app's encrypted fallback) and is never written to disk; every other
//! value is kept in a small sidecar file next to the plugin store entry, so
//! hook dispatch and plugin MCP server load can read it without a database.
//!
//! Why a sidecar file and not the plugin source manifest: a plugin **update**
//! replaces the whole installed directory (`install_plugin_source` removes it
//! first), and `userConfig` values must survive an update (spec). The sidecar
//! lives one level up, at `<plugin store>/<source_id>.options.json`, so it
//! survives an update and is removed only by an uninstall.
//!
//! Values usable as `CLAUDE_PLUGIN_OPTION_<KEY>` always come back from
//! [`load_option_values`].

use super::plugin_manifest::PluginManifest;
use crate::mcp::secret_templates::secret_store;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// File suffix of the per-source option sidecar inside a plugin store.
const OPTIONS_FILE_SUFFIX: &str = ".options.json";

/// What the sidecar holds: non-sensitive values in the clear, sensitive ones
/// only as key names (their values live in secure storage).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPluginOptions {
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    #[serde(default)]
    pub sensitive_keys: Vec<String>,
}

impl SavedPluginOptions {
    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.sensitive_keys.is_empty()
    }
}

/// Path of one source's option sidecar inside a plugin store directory.
pub fn options_path(store_dir: &Path, source_id: &str) -> PathBuf {
    store_dir.join(format!("{source_id}{OPTIONS_FILE_SUFFIX}"))
}

fn cache() -> &'static RwLock<BTreeMap<String, BTreeMap<String, String>>> {
    static CACHE: once_cell::sync::Lazy<RwLock<BTreeMap<String, BTreeMap<String, String>>>> =
        once_cell::sync::Lazy::new(|| RwLock::new(BTreeMap::new()));
    &CACHE
}

fn cache_key(store_dir: &Path, source_id: &str) -> String {
    format!("{}::{source_id}", store_dir.display())
}

fn forget_cached(store_dir: &Path, source_id: &str) {
    let key = cache_key(store_dir, source_id);
    if let Ok(mut cache) = cache().write() {
        cache.remove(&key);
    }
}

/// Secure-store account namespace for one plugin's sensitive options. The
/// `plugin:` prefix keeps these apart from MCP server secret templates.
fn secure_namespace(source_id: &str) -> String {
    format!("plugin:{source_id}")
}

/// Read a source's sidecar. A missing or unreadable file means "no options"
/// and is not an error: options are optional for every plugin.
pub fn read_saved_options(store_dir: &Path, source_id: &str) -> SavedPluginOptions {
    let path = options_path(store_dir, source_id);
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|error| {
            log::warn!("Ignoring unreadable {}: {}", path.display(), error);
            SavedPluginOptions::default()
        }),
        Err(_) => SavedPluginOptions::default(),
    }
}

/// Write a source's sidecar, creating the store directory when needed.
pub fn write_saved_options(
    store_dir: &Path,
    source_id: &str,
    options: &SavedPluginOptions,
) -> Result<(), String> {
    forget_cached(store_dir, source_id);
    std::fs::create_dir_all(store_dir)
        .map_err(|error| format!("create {}: {error}", store_dir.display()))?;
    let path = options_path(store_dir, source_id);
    let json = serde_json::to_string_pretty(options)
        .map_err(|error| format!("serialize plugin options: {error}"))?;
    std::fs::write(&path, json).map_err(|error| format!("write {}: {error}", path.display()))
}

/// Remove a source's sidecar (uninstall). A missing file is fine.
pub fn remove_saved_options(store_dir: &Path, source_id: &str) -> Result<(), String> {
    forget_cached(store_dir, source_id);
    match std::fs::remove_file(options_path(store_dir, source_id)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "remove {}: {error}",
            options_path(store_dir, source_id).display()
        )),
    }
}

/// Split resolved option values into the part the sidecar may keep and the
/// part secure storage must hold, and store the sensitive values. A sensitive
/// value with no usable secure store is an error: silently writing it to a
/// plain file would break the `sensitive` promise.
///
/// A value that was sensitive in a previous install but is not part of this
/// one is deleted from secure storage, so an unmarked option cannot linger.
pub async fn persist_options(
    source_id: &str,
    manifest: &PluginManifest,
    values: &BTreeMap<String, String>,
) -> Result<SavedPluginOptions, String> {
    let store = secret_store();
    let mut saved = SavedPluginOptions::default();
    for (key, value) in values {
        let sensitive = manifest
            .user_config
            .get(key)
            .is_some_and(|spec| spec.sensitive);
        if !sensitive {
            saved.values.insert(key.clone(), value.clone());
            continue;
        }
        if !store.available() {
            return Err(format!(
                "plugin option {key:?} is sensitive, but no secure store is available to keep it out of plain files"
            ));
        }
        if let Some(notice) = store.set(&secure_namespace(source_id), key, value).await? {
            log::warn!("plugin {source_id}: {notice}");
        }
        saved.sensitive_keys.push(key.clone());
    }
    Ok(saved)
}

/// Every option value of a source, ready to export as
/// `CLAUDE_PLUGIN_OPTION_*`: the sidecar's plain values plus the sensitive ones
/// read back from secure storage. A failed read is reported as a warning
/// string and leaves that option absent rather than failing the caller.
pub async fn load_option_values(
    store_dir: &Path,
    source_id: &str,
) -> (BTreeMap<String, String>, Option<String>) {
    let saved = read_saved_options(store_dir, source_id);
    if saved.is_empty() {
        return (BTreeMap::new(), None);
    }
    let key = cache_key(store_dir, source_id);
    if let Some(cached) = cache()
        .read()
        .ok()
        .and_then(|cache| cache.get(&key).cloned())
    {
        return (cached, None);
    }
    let mut values = saved.values.clone();
    let mut warning = None;
    let store = secret_store();
    for name in &saved.sensitive_keys {
        match store.get(&secure_namespace(source_id), name).await {
            Ok(Some(value)) => {
                values.insert(name.clone(), value);
            }
            Ok(None) => {}
            Err(error) => {
                warning = Some(format!(
                    "plugin {source_id}: could not read option {name:?} from secure storage: {error}"
                ));
            }
        }
    }
    if warning.is_none() {
        if let Ok(mut cache) = cache().write() {
            cache.insert(key, values.clone());
        }
    }
    (values, warning)
}

/// Drop the sensitive values of an uninstalled plugin. Missing entries are not
/// an error; a store failure is reported so the caller can warn.
pub async fn delete_sensitive_options(
    source_id: &str,
    sensitive_keys: &[String],
) -> Result<(), String> {
    if sensitive_keys.is_empty() {
        return Ok(());
    }
    let store = secret_store();
    if !store.available() {
        return Ok(());
    }
    for key in sensitive_keys {
        store.delete(&secure_namespace(source_id), key).await?;
    }
    Ok(())
}

/// Test-only: drop every cached value so a test starts clean.
#[cfg(test)]
pub fn clear_cache_for_test() {
    if let Ok(mut cache) = cache().write() {
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::skill_service::plugin_manifest::UserConfigSpec;

    fn manifest_with(sensitive: &[&str]) -> PluginManifest {
        let mut user_config = BTreeMap::new();
        for key in ["plain_key", "secret_key"] {
            user_config.insert(
                key.to_string(),
                UserConfigSpec {
                    sensitive: sensitive.contains(&key),
                    required: true,
                    ..Default::default()
                },
            );
        }
        PluginManifest {
            name: "p".into(),
            version: None,
            description: None,
            author: None,
            homepage: None,
            repository: None,
            license: None,
            keywords: None,
            skills: None,
            commands: None,
            agents: None,
            hooks: None,
            mcp_servers: None,
            user_config,
        }
    }

    #[tokio::test]
    async fn sensitive_values_go_to_the_secure_store_only() {
        clear_cache_for_test();
        let store_dir = tempfile::tempdir().unwrap();
        let store = store_dir.path();
        let manifest = manifest_with(&["secret_key"]);
        let values = BTreeMap::from([
            ("plain_key".to_string(), "shown".to_string()),
            ("secret_key".to_string(), "hidden".to_string()),
        ]);
        let saved = persist_options("opt-test-1", &manifest, &values)
            .await
            .unwrap();
        assert_eq!(
            saved.values.get("plain_key").map(String::as_str),
            Some("shown")
        );
        assert!(
            !saved.values.contains_key("secret_key"),
            "a sensitive value must never be persisted in plain form"
        );
        assert_eq!(saved.sensitive_keys, vec!["secret_key".to_string()]);
        write_saved_options(store, "opt-test-1", &saved).unwrap();

        let raw = std::fs::read_to_string(options_path(store, "opt-test-1")).unwrap();
        assert!(
            !raw.contains("hidden"),
            "the sidecar on disk must not contain the sensitive value: {raw}"
        );

        let (loaded, warning) = load_option_values(store, "opt-test-1").await;
        assert!(warning.is_none(), "loading must not warn: {warning:?}");
        assert_eq!(
            loaded.get("secret_key").map(String::as_str),
            Some("hidden"),
            "the sensitive value is read back from secure storage"
        );
        assert_eq!(loaded.get("plain_key").map(String::as_str), Some("shown"));

        delete_sensitive_options("opt-test-1", &saved.sensitive_keys)
            .await
            .unwrap();
        clear_cache_for_test();
        let (after, _) = load_option_values(store, "opt-test-1").await;
        assert!(
            !after.contains_key("secret_key"),
            "uninstall removes the sensitive value"
        );
        remove_saved_options(store, "opt-test-1").unwrap();
        assert!(!options_path(store, "opt-test-1").exists());
    }

    #[test]
    fn a_missing_sidecar_has_no_options() {
        let store_dir = tempfile::tempdir().unwrap();
        assert!(read_saved_options(store_dir.path(), "nope").is_empty());
    }
}
