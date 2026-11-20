//! Claude Code plugin manifest parsing (`.claude-plugin/plugin.json`, #2921).
//!
//! The manifest is optional: `None` means the plugin carries no manifest and
//! every caller keeps its current behavior. When present, `name`, `version`,
//! `description`, `author`, `homepage`, `repository`, `license`, and
//! `keywords` are stored with the plugin source, and the component path
//! fields (`skills`, `commands`, `agents`, `hooks`, `mcpServers`) must follow
//! the spec path rules: start with `./` and stay inside the plugin root.
//!
//! Per the spec, `hooks/hooks.json` (and `hooks/hooks-cursor.json`) are
//! always auto-loaded; the manifest `hooks` field lists ADDITIONAL hook
//! config files on top of those defaults (add-semantics, not replacement).
//!
//! Spec: <https://code.claude.com/docs/en/plugins-reference>

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const PLUGIN_MANIFEST_DIR: &str = ".claude-plugin";
pub const PLUGIN_MANIFEST_FILE: &str = "plugin.json";

/// A component-path field in the manifest: one path or an array of paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComponentPaths {
    Single(String),
    Many(Vec<String>),
}

impl ComponentPaths {
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            ComponentPaths::Single(path) => vec![path.clone()],
            ComponentPaths::Many(paths) => paths.clone(),
        }
    }
}

/// The `author` field: a plain name string or `{name, email?, url?}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PluginAuthor {
    Name(String),
    Info {
        name: String,
        #[serde(default)]
        email: Option<String>,
        #[serde(default)]
        url: Option<String>,
    },
}

impl PluginAuthor {
    pub fn display_name(&self) -> &str {
        match self {
            PluginAuthor::Name(name) => name,
            PluginAuthor::Info { name, .. } => name,
        }
    }
}

/// Parsed `.claude-plugin/plugin.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<PluginAuthor>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    #[serde(default)]
    pub skills: Option<ComponentPaths>,
    #[serde(default)]
    pub commands: Option<ComponentPaths>,
    #[serde(default)]
    pub agents: Option<ComponentPaths>,
    #[serde(default)]
    pub hooks: Option<ComponentPaths>,
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: Option<ComponentPaths>,
}

/// Where the manifest lives relative to a plugin root.
pub fn plugin_manifest_path(root: &Path) -> PathBuf {
    root.join(PLUGIN_MANIFEST_DIR).join(PLUGIN_MANIFEST_FILE)
}

/// Load the manifest from a plugin root. `Ok(None)` means no manifest file.
/// `Err` means a manifest exists but is not usable (bad JSON, missing or
/// unsafe `name`).
pub fn load_plugin_manifest(root: &Path) -> Result<Option<PluginManifest>, String> {
    let path = plugin_manifest_path(root);
    if !path.is_file() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let manifest: PluginManifest =
        serde_json::from_str(&raw).map_err(|error| format!("parse {}: {error}", path.display()))?;
    validate_plugin_name(&manifest.name)?;
    Ok(Some(manifest))
}

/// The manifest `name` doubles as the skill namespace, so it must be
/// non-empty and free of characters that would break `plugin:skill` names or
/// storage paths.
fn validate_plugin_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("plugin manifest name cannot be empty".to_string());
    }
    if name.contains(':') || name.contains('/') || name.contains('\\') {
        return Err(format!(
            "plugin manifest name {name:?} cannot be used as a skill namespace"
        ));
    }
    Ok(())
}

/// Spec path rule: relative to the plugin root, must start with `./`, and may
/// not traverse out of it (`..`, absolute paths, or drive/root prefixes).
pub fn validate_component_path(raw: &str) -> Result<(), String> {
    let Some(inner) = raw.strip_prefix("./") else {
        return Err(format!(
            "plugin manifest path {raw:?} must start with \"./\""
        ));
    };
    if inner.is_empty() {
        return Err(format!("plugin manifest path {raw:?} is empty"));
    }
    for component in Path::new(inner).components() {
        match component {
            Component::Normal(_) => {}
            _ => {
                return Err(format!(
                    "plugin manifest path {raw:?} must stay inside the plugin root"
                ))
            }
        }
    }
    Ok(())
}

/// Validate every component path in the manifest (#2921 AC: a path outside
/// the plugin root is a preview-time validation error).
pub fn validate_manifest_paths(manifest: &PluginManifest) -> Result<(), String> {
    let fields = [
        ("skills", &manifest.skills),
        ("commands", &manifest.commands),
        ("agents", &manifest.agents),
        ("hooks", &manifest.hooks),
        ("mcpServers", &manifest.mcp_servers),
    ];
    for (field, paths) in fields {
        if let Some(paths) = paths {
            for raw in paths.to_vec() {
                validate_component_path(&raw)
                    .map_err(|error| format!("plugin manifest {field}: {error}"))?;
            }
        }
    }
    Ok(())
}

/// Custom `hooks` file paths from the manifest. `None` means only the
/// default `hooks/hooks.json` discovery applies. When `Some`, these files
/// are loaded IN ADDITION to the defaults (the defaults are always
/// auto-loaded per the spec); a manifest entry that duplicates a default,
/// or a missing file, is a preview-time error in the caller.
pub fn hook_config_paths(manifest: Option<&PluginManifest>) -> Option<Vec<String>> {
    manifest.and_then(|m| m.hooks.as_ref()).map(|p| p.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_manifest(root: &Path, json: &str) {
        let dir = root.join(PLUGIN_MANIFEST_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(PLUGIN_MANIFEST_FILE), json).unwrap();
    }

    #[test]
    fn missing_manifest_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_plugin_manifest(dir.path()).unwrap().is_none());
    }

    #[test]
    fn parses_all_documented_fields() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
                "name": "superpowers",
                "version": "1.2.3",
                "description": "Skills for agents",
                "author": { "name": "Jesse Vincent", "email": "jesse@example.com" },
                "homepage": "https://example.com/home",
                "repository": "https://github.com/obra/superpowers",
                "license": "MIT",
                "keywords": ["skills", "agents"],
                "skills": "./skills/",
                "commands": ["./cmds/a.md", "./cmds/b.md"],
                "agents": "./agents/",
                "hooks": "./hooks/custom.json",
                "mcpServers": "./mcp.json"
            }"#,
        );
        let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
        assert_eq!(manifest.name, "superpowers");
        assert_eq!(manifest.version.as_deref(), Some("1.2.3"));
        assert_eq!(manifest.description.as_deref(), Some("Skills for agents"));
        assert_eq!(
            manifest.author.as_ref().unwrap().display_name(),
            "Jesse Vincent"
        );
        assert_eq!(
            manifest.homepage.as_deref(),
            Some("https://example.com/home")
        );
        assert_eq!(
            manifest.repository.as_deref(),
            Some("https://github.com/obra/superpowers")
        );
        assert_eq!(manifest.license.as_deref(), Some("MIT"));
        assert_eq!(
            manifest.keywords.as_deref().unwrap(),
            ["skills".to_string(), "agents".to_string()]
        );
        assert_eq!(
            manifest.skills,
            Some(ComponentPaths::Single("./skills/".into()))
        );
        assert_eq!(
            manifest.commands,
            Some(ComponentPaths::Many(vec![
                "./cmds/a.md".into(),
                "./cmds/b.md".into()
            ]))
        );
        assert_eq!(
            hook_config_paths(Some(&manifest)),
            Some(vec!["./hooks/custom.json".to_string()])
        );
        validate_manifest_paths(&manifest).unwrap();
    }

    #[test]
    fn string_author_and_missing_optional_fields() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{ "name": "p", "author": "Jane Doe" }"#);
        let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
        assert_eq!(manifest.author.as_ref().unwrap().display_name(), "Jane Doe");
        assert!(manifest.version.is_none());
        assert!(hook_config_paths(Some(&manifest)).is_none());
        validate_manifest_paths(&manifest).unwrap();
    }

    #[test]
    fn invalid_json_and_missing_name_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), "{ not json");
        assert!(load_plugin_manifest(dir.path())
            .unwrap_err()
            .contains("parse"));

        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{ "version": "1.0" }"#);
        assert!(load_plugin_manifest(dir.path())
            .unwrap_err()
            .contains("name"));
    }

    #[test]
    fn unsafe_namespace_names_are_rejected() {
        for name in ["", "  ", "a:b", "a/b", "a\\b"] {
            let dir = tempfile::tempdir().unwrap();
            let manifest = format!("{{ \"name\": {name:?} }}");
            write_manifest(dir.path(), &manifest);
            assert!(
                load_plugin_manifest(dir.path()).is_err(),
                "name {name:?} should be rejected"
            );
        }
    }

    #[test]
    fn component_path_rules() {
        assert!(validate_component_path("./hooks/hooks.json").is_ok());
        assert!(validate_component_path("./skills").is_ok());
        assert!(validate_component_path("./a/b/c.json").is_ok());
        assert!(validate_component_path("hooks/hooks.json").is_err());
        assert!(validate_component_path("./").is_err());
        assert!(validate_component_path("./../evil").is_err());
        assert!(validate_component_path("./a/../../evil").is_err());
        assert!(validate_component_path("/etc/passwd").is_err());
        assert!(validate_component_path("").is_err());
    }

    #[test]
    fn manifest_path_outside_root_fails_validation() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{ "name": "p", "hooks": "../outside.json" }"#);
        let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
        let error = validate_manifest_paths(&manifest).unwrap_err();
        assert!(error.contains("hooks"), "error names the field: {error}");
        assert!(error.contains("./"), "error explains the rule: {error}");
    }
}
