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
use std::collections::BTreeMap;
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

/// The manifest `hooks` field (#3005). Claude Code accepts a path, an inline
/// hooks object, or an array that mixes both:
/// <https://code.claude.com/docs/en/plugins/manifest-reference>
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HooksDecl {
    Single(String),
    List(Vec<HookDeclEntry>),
    /// One inline hooks object (`{ "PreToolUse": [ ... ] }`).
    Object(serde_json::Value),
}

/// One entry of a `hooks` array: a config file path or an inline hooks object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HookDeclEntry {
    Path(String),
    Inline(serde_json::Value),
}

impl HooksDecl {
    /// The config file paths this field declares, in order.
    pub fn paths(&self) -> Vec<&str> {
        match self {
            HooksDecl::Single(path) => vec![path.as_str()],
            HooksDecl::Object(_) => Vec::new(),
            HooksDecl::List(entries) => entries
                .iter()
                .filter_map(|entry| match entry {
                    HookDeclEntry::Path(path) => Some(path.as_str()),
                    HookDeclEntry::Inline(_) => None,
                })
                .collect(),
        }
    }

    /// The inline hooks objects this field declares, in order.
    pub fn inline_values(&self) -> Vec<&serde_json::Value> {
        match self {
            HooksDecl::Single(_) => Vec::new(),
            HooksDecl::Object(value) => vec![value],
            HooksDecl::List(entries) => entries
                .iter()
                .filter_map(|entry| match entry {
                    HookDeclEntry::Inline(value) => Some(value),
                    HookDeclEntry::Path(_) => None,
                })
                .collect(),
        }
    }
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

/// The manifest `mcpServers` field (#2924): one path or an array of paths
/// to server-declaration files, or an inline `{name: entry}` map. Untagged
/// so an object value falls through to the `Inline` arm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpServersDecl {
    Paths(ComponentPaths),
    Inline(std::collections::BTreeMap<String, serde_json::Value>),
}

/// One entry of the manifest `userConfig` map (#2926): a value the plugin
/// needs from the user before it can run. The spec defines `type`, `title`,
/// `description`, `sensitive`, `required`, and `default`.
/// Spec: <https://code.claude.com/docs/en/plugins-reference> (User
/// configuration).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserConfigSpec {
    /// `string` | `number` | `boolean` | `directory` | `file`. Unknown values
    /// are kept as-is and treated as `string`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// A sensitive value is asked for with masked input and stored in the OS
    /// keychain, never in the plugin manifest.
    #[serde(default)]
    pub sensitive: bool,
    #[serde(default)]
    pub required: bool,
    /// Default shown to the user and used when the value is optional and
    /// empty. Scalars (string/number/bool) are stored in their string form.
    #[serde(default, deserialize_with = "de_scalar_string")]
    pub default: Option<String>,
}

impl UserConfigSpec {
    /// Human label for the option, falling back to the key.
    pub fn display_title<'a>(&'a self, key: &'a str) -> &'a str {
        self.title.as_deref().unwrap_or(key)
    }
}

/// Accept a string, number, or boolean `default` and keep it as a string
/// (the form an environment variable needs).
fn de_scalar_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde_json::Value;
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text),
        Some(Value::Bool(flag)) => Some(flag.to_string()),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(other) => Some(other.to_string()),
    })
}

/// Environment-variable name a plugin option is exported as
/// (`CLAUDE_PLUGIN_OPTION_<KEY>`, upper-cased with non-alphanumerics mapped
/// to `_`). `None` when the key has no usable alphanumeric characters.
pub fn option_env_name(key: &str) -> Option<String> {
    let mut name = String::with_capacity(key.len());
    for ch in key.chars() {
        if ch.is_ascii_alphanumeric() {
            name.push(ch.to_ascii_uppercase());
        } else {
            name.push('_');
        }
    }
    let trimmed = name.trim_matches('_');
    if trimmed.is_empty() {
        return None;
    }
    Some(format!("CLAUDE_PLUGIN_OPTION_{trimmed}"))
}

/// `${user_config.KEY}` placeholder form used inside hook and MCP
/// declarations.
pub fn user_config_placeholder(key: &str) -> String {
    format!("${{user_config.{key}}}")
}

/// Parsed `.claude-plugin/plugin.json`.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Hook config: file paths, inline hooks objects, or a mix (#3005).
    #[serde(default)]
    pub hooks: Option<HooksDecl>,
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: Option<McpServersDecl>,
    /// `userConfig`: values the plugin needs from the user (#2926). Empty
    /// when the plugin declares none.
    #[serde(default, rename = "userConfig")]
    pub user_config: BTreeMap<String, UserConfigSpec>,
    /// Spec `defaultEnabled` (#2927): the state a fresh install starts in.
    /// Absent means enabled, which is what every plugin did before #2927.
    #[serde(default, rename = "defaultEnabled")]
    pub default_enabled: Option<bool>,
}

impl PluginManifest {
    /// The enabled state a fresh install starts in (#2927).
    pub fn starts_enabled(&self) -> bool {
        self.default_enabled.unwrap_or(true)
    }
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
    ];
    for (field, paths) in fields {
        if let Some(paths) = paths {
            for raw in paths.to_vec() {
                validate_component_path(&raw)
                    .map_err(|error| format!("plugin manifest {field}: {error}"))?;
            }
        }
    }
    if let Some(hooks) = &manifest.hooks {
        for raw in hooks.paths() {
            validate_component_path(raw)
                .map_err(|error| format!("plugin manifest hooks: {error}"))?;
        }
    }
    if let Some(McpServersDecl::Paths(paths)) = &manifest.mcp_servers {
        for raw in paths.to_vec() {
            validate_component_path(&raw)
                .map_err(|error| format!("plugin manifest mcpServers: {error}"))?;
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
    manifest
        .and_then(|m| m.hooks.as_ref())
        .map(|hooks| hooks.paths().into_iter().map(str::to_string).collect())
        .filter(|paths: &Vec<String>| !paths.is_empty())
}

/// Inline hooks objects the manifest declares (#3005). Each value has the same
/// shape as a hook config file's `"hooks"` member.
pub fn hook_inline_values(manifest: Option<&PluginManifest>) -> Vec<serde_json::Value> {
    manifest
        .and_then(|m| m.hooks.as_ref())
        .map(|hooks| hooks.inline_values().into_iter().cloned().collect())
        .unwrap_or_default()
}

/// Validate the `userConfig` map (#2926): every key must map to a usable
/// `CLAUDE_PLUGIN_OPTION_*` name, no two keys may collide after that mapping,
/// and a declared `type` must be one the spec defines.
pub fn validate_user_config(manifest: &PluginManifest) -> Result<(), String> {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for (key, spec) in &manifest.user_config {
        let name = option_env_name(key).ok_or_else(|| {
            format!("plugin userConfig key {key:?} cannot be exported as an environment variable")
        })?;
        if let Some(previous) = seen.insert(name.clone(), key.clone()) {
            return Err(format!(
                "plugin userConfig keys {previous:?} and {key:?} both export {name}"
            ));
        }
        if let Some(value_type) = spec.value_type.as_deref() {
            if !matches!(
                value_type,
                "string" | "number" | "boolean" | "directory" | "file"
            ) {
                return Err(format!(
                    "plugin userConfig {key:?} has unknown type {value_type:?}"
                ));
            }
        }
    }
    Ok(())
}

/// Resolve the values of a manifest's `userConfig` from what the user entered
/// (#2926): a declared default fills an omitted optional option, a `required`
/// option without a value is an error, and `number`/`boolean` values must have
/// the declared shape. Unknown keys in `provided` are dropped — an install
/// never persists a value the manifest does not declare.
pub fn resolve_user_config_values(
    manifest: &PluginManifest,
    provided: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for (key, spec) in &manifest.user_config {
        let entered = provided
            .get(key)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let value = match entered {
            Some(value) => value,
            None => match spec.default.as_ref().map(|d| d.trim()) {
                Some(default) if !default.is_empty() => default.to_string(),
                _ if spec.required => return Err(format!("plugin option {key:?} is required")),
                _ => continue,
            },
        };
        match spec.value_type.as_deref() {
            Some("number") if value.parse::<f64>().is_err() => {
                return Err(format!("plugin option {key:?} must be a number"));
            }
            Some("boolean") if !matches!(value.as_str(), "true" | "false") => {
                return Err(format!("plugin option {key:?} must be true or false"));
            }
            _ => {}
        }
        out.insert(key.clone(), value);
    }
    Ok(out)
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
        assert!(hook_inline_values(Some(&manifest)).is_empty());
        validate_manifest_paths(&manifest).unwrap();
    }

    /// `hooks` may be an inline object instead of a path (#3005).
    #[test]
    fn inline_hooks_object_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
              "name": "p",
              "hooks": { "PreToolUse": [ { "matcher": "Bash", "command": "echo hi" } ] }
            }"#,
        );
        let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
        assert_eq!(hook_config_paths(Some(&manifest)), None);
        let inline = hook_inline_values(Some(&manifest));
        assert_eq!(inline.len(), 1);
        assert!(inline[0].get("PreToolUse").is_some());
        validate_manifest_paths(&manifest).unwrap();
    }

    /// A `hooks` array may mix file paths and inline objects (#3005).
    #[test]
    fn hooks_array_mixes_paths_and_inline_objects() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
              "name": "p",
              "hooks": [
                "./hooks/custom.json",
                { "SessionEnd": [ { "command": "echo bye" } ] }
              ]
            }"#,
        );
        let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
        assert_eq!(
            hook_config_paths(Some(&manifest)),
            Some(vec!["./hooks/custom.json".to_string()])
        );
        let inline = hook_inline_values(Some(&manifest));
        assert_eq!(inline.len(), 1);
        assert!(inline[0].get("SessionEnd").is_some());
        validate_manifest_paths(&manifest).unwrap();
    }

    /// A path outside the plugin root is still rejected when it sits in a mixed
    /// `hooks` array (#3005 keeps the #2921 path rule).
    #[test]
    fn manifest_hooks_path_outside_root_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{ "name": "p", "hooks": ["./hooks/a.json", "./../escape.json"] }"#,
        );
        let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
        let error = validate_manifest_paths(&manifest).unwrap_err();
        assert!(error.contains("inside the plugin root"), "{error}");
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
