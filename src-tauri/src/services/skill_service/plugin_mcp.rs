//! Plugin-declared MCP servers (#2924).
//!
//! A plugin declares servers in `.mcp.json` at its root and/or through the
//! `.claude-plugin/plugin.json` `mcpServers` field (path file(s) or an inline
//! map). Files load in that order and a server name declared later replaces
//! an earlier one (spec: plugins-reference). The raw declared entry is kept
//! in the plugin source manifest so `${CLAUDE_PLUGIN_ROOT}` can be expanded
//! against the installed root.
//!
//! Scope rules (owner decisions D1/D2): a **global** plugin's approved
//! servers are installed into the global MCP server store as rows marked
//! `pluginSource: "plugin:<source_id>"` (read-only, removed only with the
//! plugin); a **session-scoped** plugin's servers are never copied there —
//! session load reads them straight from the session's plugin store.

use super::plugin_manifest::{validate_component_path, McpServersDecl, PluginManifest};
use crate::mcp::types::PLUGIN_SOURCE_KEY;
use crate::mcp::utils::sanitize_mcp_server_name;
use crate::services::hooks::registry::PluginMcpServerSnapshot;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

/// Default server-declaration file at the plugin root (Claude Code spec).
pub const PLUGIN_MCP_FILE: &str = ".mcp.json";

/// Placeholder names expanded to the plugin's installed root. Same alias set
/// the hook command resolver uses (`hooks/registry.rs`).
const PLUGIN_ROOT_PLACEHOLDERS: [&str; 6] = [
    "${CLAUDE_PLUGIN_ROOT}",
    "${LIBRAGENT_PLUGIN_ROOT}",
    "${CURSOR_PLUGIN_ROOT}",
    "$CLAUDE_PLUGIN_ROOT",
    "$LIBRAGENT_PLUGIN_ROOT",
    "$CURSOR_PLUGIN_ROOT",
];

/// Placeholder names expanded to the plugin's `${CLAUDE_PLUGIN_DATA}`
/// directory (#2926).
const PLUGIN_DATA_PLACEHOLDERS: [&str; 4] = [
    "${CLAUDE_PLUGIN_DATA}",
    "$CLAUDE_PLUGIN_DATA",
    "${LIBRAGENT_PLUGIN_DATA}",
    "$LIBRAGENT_PLUGIN_DATA",
];

/// Shared empty option map for expansions that carry no `userConfig` values.
#[cfg(test)]
fn no_options() -> &'static BTreeMap<String, String> {
    static EMPTY: once_cell::sync::Lazy<BTreeMap<String, String>> =
        once_cell::sync::Lazy::new(BTreeMap::new);
    &EMPTY
}

/// What a plugin-declared MCP server entry may reference (#2926): the
/// installed root, the plugin data directory, and the resolved `userConfig`
/// values (`${user_config.KEY}` and `CLAUDE_PLUGIN_OPTION_*`).
///
/// `CLAUDE_PROJECT_DIR` is deliberately absent: an MCP server config is
/// loaded per server (global store) as well as per session, so it has no
/// single project directory. The spec defines it for hooks, where it is set.
pub struct PluginExpansion<'a> {
    pub root: &'a Path,
    pub data_dir: Option<&'a Path>,
    pub options: &'a BTreeMap<String, String>,
}

impl<'a> PluginExpansion<'a> {
    /// Root-only expansion (no plugin data, no options).
    #[cfg(test)]
    pub fn root_only(root: &'a Path) -> Self {
        Self {
            root,
            data_dir: None,
            options: no_options(),
        }
    }

    fn replacements(&self) -> Vec<(String, String)> {
        let root = self.root.to_string_lossy().to_string();
        let mut pairs: Vec<(String, String)> = PLUGIN_ROOT_PLACEHOLDERS
            .iter()
            .map(|placeholder| ((*placeholder).to_string(), root.clone()))
            .collect();
        if let Some(data_dir) = self.data_dir {
            let data = data_dir.to_string_lossy().to_string();
            pairs.extend(
                PLUGIN_DATA_PLACEHOLDERS
                    .iter()
                    .map(|placeholder| ((*placeholder).to_string(), data.clone())),
            );
        }
        pairs.extend(self.options.iter().map(|(key, value)| {
            (
                super::plugin_manifest::user_config_placeholder(key),
                value.clone(),
            )
        }));
        pairs
    }
}

/// Server names in declaration order plus the raw declared entry, later
/// declarations replacing earlier ones by name.
pub fn collect_plugin_mcp_servers(
    scan_root: &Path,
    manifest: Option<&PluginManifest>,
) -> Result<Vec<PluginMcpServerSnapshot>, String> {
    let mut order: Vec<String> = Vec::new();
    let mut by_name: HashMap<String, Value> = HashMap::new();

    let mut merge_entries = |file: &Path, source_label: String| -> Result<(), String> {
        let raw =
            std::fs::read_to_string(file).map_err(|e| format!("read {}: {e}", file.display()))?;
        let parsed: Value =
            serde_json::from_str(&raw).map_err(|e| format!("parse {source_label}: {e}"))?;
        let entries = parse_declared_map(&parsed, &source_label)?;
        for (name, entry) in entries {
            validate_entry_shape(name, entry, &source_label)?;
            if !by_name.contains_key(name.as_str()) {
                order.push(name.clone());
            }
            by_name.insert(name.clone(), entry.clone());
        }
        Ok(())
    };

    let default_file = scan_root.join(PLUGIN_MCP_FILE);
    if default_file
        .symlink_metadata()
        .is_ok_and(|m| m.is_symlink())
    {
        return Err(format!(
            "{PLUGIN_MCP_FILE}: symlinks are refused in a plugin source (gatekeeper S1.1)"
        ));
    }
    if default_file.is_file() {
        merge_entries(&default_file, PLUGIN_MCP_FILE.to_string())?;
    }

    if let Some(decl) = manifest.and_then(|m| m.mcp_servers.as_ref()) {
        match decl {
            McpServersDecl::Inline(entries) => {
                for (name, entry) in entries {
                    validate_entry_shape(name, entry, "manifest mcpServers")?;
                    if !by_name.contains_key(name) {
                        order.push(name.clone());
                    }
                    by_name.insert(name.clone(), entry.clone());
                }
            }
            McpServersDecl::Paths(paths) => {
                for raw in paths.to_vec() {
                    // Same hostile-path rule as the other manifest component
                    // paths (#2921): `./`-relative, no traversal, and no
                    // symlink that could read a file from outside the plugin
                    // root (gatekeeper S1.1).
                    validate_component_path(&raw)
                        .map_err(|error| format!("plugin manifest mcpServers: {error}"))?;
                    let resolved = scan_root.join(&raw);
                    if resolved.symlink_metadata().is_ok_and(|m| m.is_symlink()) {
                        return Err(format!(
                            "plugin manifest mcpServers path {raw:?} is a symlink; refused"
                        ));
                    }
                    if !resolved.is_file() {
                        return Err(format!(
                            "plugin manifest mcpServers path {raw:?} not found in the plugin"
                        ));
                    }
                    merge_entries(&resolved, raw)?;
                }
            }
        }
    }

    Ok(order
        .into_iter()
        .filter_map(|name| {
            by_name
                .remove(&name)
                .map(|entry| PluginMcpServerSnapshot { name, entry })
        })
        .collect())
}

/// A declaration file is either the spec `{"mcpServers": {name: entry}}`
/// wrapper or a bare `{name: entry}` map.
fn parse_declared_map<'a>(
    parsed: &'a Value,
    source_label: &str,
) -> Result<&'a Map<String, Value>, String> {
    if let Some(Value::Object(map)) = parsed.get("mcpServers") {
        return Ok(map);
    }
    parsed
        .as_object()
        .ok_or_else(|| format!("{source_label}: expected a JSON object of MCP servers"))
}

fn validate_entry_shape(name: &str, entry: &Value, source_label: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err(format!("{source_label}: server name cannot be empty"));
    }
    let obj = entry
        .as_object()
        .ok_or_else(|| format!("{source_label}: server {name:?} must be a JSON object"))?;
    let has_command = obj
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| !c.trim().is_empty());
    let has_url = obj
        .get("url")
        .and_then(Value::as_str)
        .is_some_and(|u| !u.trim().is_empty());
    if !has_command && !has_url {
        return Err(format!(
            "{source_label}: server {name:?} declares neither a stdio `command` nor an http `url`"
        ));
    }
    Ok(())
}

/// Expand the plugin-root placeholders in every string of `entry` (command,
/// args, env values, url, headers — wherever they appear).
#[cfg(test)]
pub fn resolve_plugin_root(entry: &Value, plugin_root: &Path) -> Value {
    resolve_plugin_placeholders(entry, &PluginExpansion::root_only(plugin_root))
}

/// Expand every placeholder a plugin entry may use (#2926): the root aliases,
/// `${CLAUDE_PLUGIN_DATA}`, and `${user_config.KEY}`.
pub fn resolve_plugin_placeholders(entry: &Value, expansion: &PluginExpansion<'_>) -> Value {
    let pairs = expansion.replacements();
    resolve_value(entry, &pairs)
}

fn resolve_value(value: &Value, pairs: &[(String, String)]) -> Value {
    match value {
        Value::String(text) => {
            let mut expanded = text.clone();
            for (placeholder, replacement) in pairs {
                if expanded.contains(placeholder.as_str()) {
                    expanded = expanded.replace(placeholder.as_str(), replacement);
                }
            }
            Value::String(expanded)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| resolve_value(item, pairs))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, item)| (key.clone(), resolve_value(item, pairs)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Convert one resolved plugin entry into the `MCPServerConfig` JSON shape
/// the global MCP server store keeps (`{"name", "transport": {...}}`).
#[cfg(test)]
pub fn entry_to_mcp_config_json(
    name: &str,
    resolved_entry: &Value,
    source_id: &str,
) -> Result<Value, String> {
    entry_to_mcp_config_json_with(name, resolved_entry, source_id, no_options())
}

/// As [`entry_to_mcp_config_json`], plus the plugin's `userConfig` values
/// exported to a stdio server as `CLAUDE_PLUGIN_OPTION_<KEY>` (#2926). An
/// entry that declares the same variable itself keeps its own value.
pub fn entry_to_mcp_config_json_with(
    name: &str,
    resolved_entry: &Value,
    source_id: &str,
    options: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let obj = resolved_entry
        .as_object()
        .ok_or_else(|| format!("plugin MCP server {name:?} entry is not an object"))?;
    let mut transport = Map::new();
    if let Some(url) = obj.get("url").and_then(Value::as_str) {
        transport.insert("type".into(), Value::String("http".into()));
        transport.insert("url".into(), Value::String(url.to_string()));
        if let Some(Value::Object(headers)) = obj.get("headers") {
            transport.insert("headers".into(), Value::Object(headers.clone()));
        }
    } else {
        let command = obj
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("plugin MCP server {name:?} has no command"))?;
        transport.insert("type".into(), Value::String("stdio".into()));
        transport.insert("command".into(), Value::String(command.to_string()));
        let args = obj
            .get("args")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        transport.insert("args".into(), Value::Array(args));
        let mut env = match obj.get("env") {
            Some(Value::Object(env)) => env.clone(),
            _ => Map::new(),
        };
        for (key, value) in options {
            if let Some(env_name) = super::plugin_manifest::option_env_name(key) {
                env.entry(env_name).or_insert(Value::String(value.clone()));
            }
        }
        transport.insert("env".into(), Value::Object(env));
    }
    let mut config = Map::new();
    config.insert("name".into(), Value::String(name.to_string()));
    config.insert("transport".into(), Value::Object(transport));
    config.insert(
        PLUGIN_SOURCE_KEY.into(),
        Value::String(plugin_source_marker(source_id)),
    );
    Ok(Value::Object(config))
}

/// The `pluginSource` marker value for a plugin's servers (D2).
pub fn plugin_source_marker(source_id: &str) -> String {
    format!("plugin:{source_id}")
}

/// Build the config JSON for one snapshot with the full plugin expansion
/// (root, `${CLAUDE_PLUGIN_DATA}`, and `userConfig` values, #2926).
pub fn snapshot_to_mcp_config_json_in(
    snapshot: &PluginMcpServerSnapshot,
    expansion: &PluginExpansion<'_>,
    source_id: &str,
) -> Result<Value, String> {
    let resolved = resolve_plugin_placeholders(&snapshot.entry, expansion);
    entry_to_mcp_config_json_with(&snapshot.name, &resolved, source_id, expansion.options)
}

/// Create the plugin-owned rows for `servers` in the global MCP server
/// store. Callers must remove any rows previously owned by `source_id`
/// first (see [`remove_global_plugin_mcp_servers`]) so re-install and
/// update converge exactly on `servers` (gatekeeper G1.1).
///
/// Returns one warning per server that could not be installed; the caller
/// must surface them to the user — the user approved these servers, so a
/// silent drop is not acceptable (gatekeeper G1.2). A failure to read or
/// write the store itself is an `Err`.
#[cfg(test)]
pub async fn create_plugin_mcp_server_rows(
    source_id: &str,
    plugin_root: &Path,
    servers: &[PluginMcpServerSnapshot],
) -> Result<Vec<String>, String> {
    create_plugin_mcp_server_rows_in(source_id, &PluginExpansion::root_only(plugin_root), servers)
        .await
}

/// As [`create_plugin_mcp_server_rows`], with the plugin's data directory and
/// `userConfig` values available to each entry (#2926).
pub async fn create_plugin_mcp_server_rows_in(
    source_id: &str,
    expansion: &PluginExpansion<'_>,
    servers: &[PluginMcpServerSnapshot],
) -> Result<Vec<String>, String> {
    use crate::services::{McpServerService, SaveVerification};
    let repo = crate::state::get_mcp_server_repository();

    let mut warnings = Vec::new();
    for snapshot in servers {
        let name = sanitize_mcp_server_name(&snapshot.name);
        let outcome: Result<String, String> = async {
            let config = snapshot_to_mcp_config_json_in(snapshot, expansion, source_id)?;
            let model = McpServerService::create_server_config(
                repo,
                name.clone(),
                config,
                SaveVerification::Pending,
            )
            .await?;
            Ok(model.id)
        }
        .await;
        match outcome {
            Ok(id) => McpServerService::schedule_background_probe(id),
            Err(error) => {
                log::warn!(
                    "Plugin {source_id}: could not install MCP server '{}' into the global store: {}",
                    snapshot.name,
                    error
                );
                warnings.push(format!(
                    "Plugin {source_id}: MCP server '{}' was approved but could not be installed into the global store: {error}",
                    snapshot.name
                ));
            }
        }
    }
    Ok(warnings)
}

/// Delete every global MCP server row owned by `source_id` (plugin uninstall).
pub async fn remove_global_plugin_mcp_servers(source_id: &str) -> Result<(), String> {
    use crate::repositories::mcp_server_repository::MCPServerRepository;
    use crate::services::mcp_server_service::McpServerService;
    let repo = crate::state::get_mcp_server_repository();
    let marker = plugin_source_marker(source_id);
    let models = repo
        .list()
        .await
        .map_err(|e| format!("list MCP servers: {e}"))?;
    for model in models {
        if crate::mcp::types::plugin_source_of(&model.config).as_deref() == Some(marker.as_str()) {
            McpServerService::delete_plugin_owned_server(&model.id).await?;
        }
    }
    Ok(())
}

/// Approved MCP servers effective for one session, from the session's own
/// plugin store only (#2924 D1: session servers never enter the global
/// store). Returns `(source_id, snapshot)` pairs plus the map of all source
/// ids present in the session store (approved or not) with their installed
/// roots — the same-id set that shadows global plugins and resolves
/// `${CLAUDE_PLUGIN_ROOT}`. Unapproved snapshots are skipped.
///
/// Approval is read through [`list_session_store_sources`], i.e. from the
/// out-of-workspace trust record overlaid on the store manifest (#2942
/// S3.2): the store now lives in the agent-writable session workspace, so
/// its own `mcp_approved` field is never trusted. `trust_dir: None` means
/// no approval can exist — every server is skipped.
/// Approved MCP servers effective for one session, keeping each one's whole
/// source record so load can also apply the plugin's `userConfig` values and
/// `${CLAUDE_PLUGIN_DATA}` (#2926).
pub fn session_plugin_mcp_records(
    session_store: &Path,
    trust_dir: Option<&Path>,
) -> (
    Vec<(
        crate::services::hooks::registry::PluginSourceRecord,
        PluginMcpServerSnapshot,
    )>,
    BTreeMap<String, std::path::PathBuf>,
) {
    use crate::services::hooks::registry::list_session_store_sources;
    let records = list_session_store_sources(trust_dir, session_store);
    let mut out = Vec::new();
    let mut roots = BTreeMap::new();
    for record in records {
        let source_id = record.manifest.source_id.clone();
        roots.insert(source_id, record.root_dir.clone());
        // #2927: a disabled plugin contributes no servers, approved or not.
        if !record.manifest.mcp_approved || !record.manifest.enabled {
            continue;
        }
        for snapshot in record.manifest.mcp_servers.clone() {
            out.push((record.clone(), snapshot));
        }
    }
    (out, roots)
}

/// One session-scoped plugin MCP server resolved for session load (#2924 D1):
/// the store config JSON (plugin-root placeholders expanded, plugin-source
/// marker set) plus the runtime name and pseudo-id the session proxy keys it
/// under (`plugin:<source_id>:<name>` — no DB row exists for these).
pub struct SessionPluginServerConfig {
    pub server_name: String,
    pub pseudo_id: String,
    pub config: crate::mcp::types::MCPServerConfig,
}

/// Load every approved session-plugin server from one session plugin store
/// into ready-to-connect configs (the D1 loader used by
/// `load_requested_server_configs`). Invalid entries are skipped with a
/// warning; `taken_session_names` keeps runtime names unique across the
/// session's servers, same as DB-loaded servers. `trust_dir` is the
/// out-of-workspace trust record directory for this session (#2942 S3.2).
pub async fn build_session_plugin_server_configs(
    session_store: &Path,
    session_id: &str,
    trust_dir: Option<&Path>,
    taken_session_names: &mut HashSet<String>,
) -> Vec<SessionPluginServerConfig> {
    use crate::mcp::types::MCPServerConfig;
    use crate::mcp::utils::unique_session_server_name;

    let (records, _roots) = session_plugin_mcp_records(session_store, trust_dir);
    let mut out = Vec::new();
    for (record, snapshot) in records {
        let source_id = record.manifest.source_id.clone();
        let root = record.root_dir.clone();
        // #2926: session-scope plugins get their session data dir and their
        // resolved `userConfig` values, exactly like their hooks do. The
        // option sidecar is read from the trust directory, never from the
        // agent-writable workspace store (#2926 gatekeeper G1.1), and the
        // secure namespace is scoped to this session (G1.2).
        let (options, warning) = super::plugin_options::load_option_values(
            trust_dir.unwrap_or(session_store),
            &source_id,
            Some(session_id),
        )
        .await;
        if let Some(warning) = warning {
            log::warn!("{warning}");
        }
        let data_dir =
            crate::services::hooks::registry::get_session_plugin_data_dir(session_id, &source_id)
                .ok();
        let config_json = match snapshot_to_mcp_config_json_in(
            &snapshot,
            &PluginExpansion {
                root: &root,
                data_dir: data_dir.as_deref(),
                options: &options,
            },
            &source_id,
        ) {
            Ok(json) => json,
            Err(error) => {
                log::warn!(
                    "Session {}: plugin {} MCP server '{}' skipped: {}",
                    session_id,
                    source_id,
                    snapshot.name,
                    error
                );
                continue;
            }
        };
        let mut config = match serde_json::from_value::<MCPServerConfig>(config_json) {
            Ok(config) => config,
            Err(error) => {
                log::warn!(
                    "Session {}: plugin {} MCP server '{}' invalid config skipped: {}",
                    session_id,
                    source_id,
                    snapshot.name,
                    error
                );
                continue;
            }
        };
        let pseudo_id = format!(
            "{}{}:{}",
            crate::mcp::types::PLUGIN_SOURCE_PREFIX,
            source_id,
            snapshot.name
        );
        let server_name =
            unique_session_server_name(&snapshot.name, &pseudo_id, taken_session_names);
        config.name = Some(server_name.clone());
        out.push(SessionPluginServerConfig {
            server_name,
            pseudo_id,
            config,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::skill_service::plugin_manifest::load_plugin_manifest;
    use std::fs;

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn default_file_wrapper_and_bare_map_parse() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            ".mcp.json",
            r#"{"mcpServers": {"fetch": {"command": "uvx", "args": ["mcp-fetch"]}}}"#,
        );
        let servers = collect_plugin_mcp_servers(dir.path(), None).unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "fetch");
        assert_eq!(servers[0].entry["command"], "uvx");

        let dir2 = tempfile::tempdir().unwrap();
        write(
            dir2.path(),
            ".mcp.json",
            r#"{"bare": {"url": "https://example.com/mcp"}}"#,
        );
        let servers = collect_plugin_mcp_servers(dir2.path(), None).unwrap();
        assert_eq!(servers[0].name, "bare");
        assert_eq!(servers[0].entry["url"], "https://example.com/mcp");
    }

    #[test]
    fn manifest_paths_replace_default_by_name() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            ".mcp.json",
            r#"{"mcpServers": {
                 "keep": {"command": "keep-cmd"},
                 "dup": {"command": "old-cmd"}
               }}"#,
        );
        write(
            dir.path(),
            ".claude-plugin/plugin.json",
            r#"{"name": "p", "mcpServers": "./extra.json"}"#,
        );
        write(
            dir.path(),
            "extra.json",
            r#"{"mcpServers": {"dup": {"command": "new-cmd"}, "added": {"url": "https://x/mcp"}}}"#,
        );
        let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
        let servers = collect_plugin_mcp_servers(dir.path(), Some(&manifest)).unwrap();
        let names: Vec<&str> = servers.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["keep", "dup", "added"]);
        assert_eq!(servers[1].entry["command"], "new-cmd");
    }

    #[test]
    fn manifest_inline_map_supported() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            ".claude-plugin/plugin.json",
            r#"{"name": "p", "mcpServers": {"inlined": {"command": "run", "args": ["--stdio"]}}}"#,
        );
        let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
        let servers = collect_plugin_mcp_servers(dir.path(), Some(&manifest)).unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "inlined");
        assert_eq!(servers[0].entry["args"][0], "--stdio");
    }

    #[test]
    fn missing_manifest_path_and_bad_shape_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            ".claude-plugin/plugin.json",
            r#"{"name": "p", "mcpServers": "./nope.json"}"#,
        );
        let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
        let error = collect_plugin_mcp_servers(dir.path(), Some(&manifest)).unwrap_err();
        assert!(error.contains("nope.json"), "{error}");

        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), ".mcp.json", r#"{"bad": {"noTransport": true}}"#);
        let error = collect_plugin_mcp_servers(dir.path(), None).unwrap_err();
        assert!(error.contains("bad"), "{error}");
        assert!(error.contains("command"), "{error}");
    }

    // #2924 (gatekeeper S1.1): symlinked MCP declaration files are refused —
    // a symlink could read a file from outside the plugin root.
    #[cfg(unix)]
    #[test]
    fn symlinked_declaration_files_are_refused() {
        use std::os::unix::fs::symlink;

        // `.mcp.json` itself is a symlink.
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("outside.json"),
            r#"{"srv":{"command":"x"}}"#,
        )
        .unwrap();
        symlink(
            dir.path().join("outside.json"),
            dir.path().join(".mcp.json"),
        )
        .unwrap();
        let error = collect_plugin_mcp_servers(dir.path(), None).unwrap_err();
        assert!(error.contains("symlink"), "{error}");

        // A manifest `mcpServers` path resolves to a symlink.
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            ".claude-plugin/plugin.json",
            r#"{"name":"p","mcpServers":"./linked.json"}"#,
        );
        fs::write(
            dir.path().join("outside.json"),
            r#"{"srv":{"command":"x"}}"#,
        )
        .unwrap();
        symlink(
            dir.path().join("outside.json"),
            dir.path().join("linked.json"),
        )
        .unwrap();
        let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
        let error = collect_plugin_mcp_servers(dir.path(), Some(&manifest)).unwrap_err();
        assert!(error.contains("symlink"), "{error}");
    }

    // #2921 hostile-path rule also guards manifest `mcpServers` paths.
    #[test]
    fn manifest_mcp_servers_path_traversal_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            ".claude-plugin/plugin.json",
            r#"{"name":"p","mcpServers":"./../evil.json"}"#,
        );
        let manifest = load_plugin_manifest(dir.path()).unwrap().expect("manifest");
        let error = collect_plugin_mcp_servers(dir.path(), Some(&manifest)).unwrap_err();
        assert!(error.contains("mcpServers"), "{error}");
    }

    #[test]
    fn plugin_root_placeholders_resolve_everywhere() {
        let dir = tempfile::tempdir().unwrap();
        let entry = serde_json::json!({
            "command": "${CLAUDE_PLUGIN_ROOT}/bin/run",
            "args": ["--root", "${LIBRAGENT_PLUGIN_ROOT}/cfg"],
            "env": {"ROOT": "$CURSOR_PLUGIN_ROOT/data"}
        });
        let resolved = resolve_plugin_root(&entry, dir.path());
        let root = dir.path().to_string_lossy();
        assert_eq!(resolved["command"], format!("{root}/bin/run"));
        assert_eq!(resolved["args"][1], format!("{root}/cfg"));
        assert_eq!(resolved["env"]["ROOT"], format!("{root}/data"));
    }

    #[test]
    fn entry_converts_to_stored_config_shape() {
        let entry = serde_json::json!({
            "command": "node",
            "args": ["/server/index.js"],
            "env": {"A": "1"}
        });
        let config = entry_to_mcp_config_json("srv", &entry, "plugin-0123456789abcdef").unwrap();
        assert_eq!(config["transport"]["type"], "stdio");
        assert_eq!(config["transport"]["command"], "node");
        assert_eq!(config["transport"]["env"]["A"], "1");
        assert_eq!(config["pluginSource"], "plugin:plugin-0123456789abcdef");

        let http_entry =
            serde_json::json!({"url": "https://mcp.example.com", "headers": {"k": "v"}});
        let config = entry_to_mcp_config_json("remote", &http_entry, "plugin-x").unwrap();
        assert_eq!(config["transport"]["type"], "http");
        assert_eq!(config["transport"]["url"], "https://mcp.example.com");
        assert_eq!(config["transport"]["headers"]["k"], "v");

        // The produced JSON parses back as a valid MCPServerConfig.
        let parsed: crate::mcp::types::MCPServerConfig =
            serde_json::from_value(config).expect("valid MCPServerConfig");
        match parsed.transport {
            crate::mcp::types::TransportConfig::Http { url, .. } => {
                assert_eq!(url, "https://mcp.example.com")
            }
            _ => panic!("expected http"),
        }
    }

    /// Write a session-store source record the way `save_source_in` does:
    /// `<base>/<source_id>/manifest.json` with a matching `source_id`.
    fn write_store_manifest(
        base: &Path,
        source_id: &str,
        mcp_approved: bool,
        servers_json: &str,
    ) -> std::path::PathBuf {
        let root = base.join(source_id);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("manifest.json"),
            format!(
                r#"{{
                  "source_url": "https://example.com/{source_id}",
                  "source_id": "{source_id}",
                  "skill_names": [],
                  "hooks": null,
                  "hooks_approved": false,
                  "approval_hashes": {{}},
                  "created_at": 0,
                  "plugin": null,
                  "hook_files": [],
                  "mcp_servers": {servers_json},
                  "mcp_approved": {mcp_approved},
                  "enabled": true,
                  "installed_commit": null
                }}"#
            ),
        )
        .unwrap();
        root
    }

    // #2924 (gatekeeper G1.4): session load reads only approved servers from
    // the session store, resolves `${CLAUDE_PLUGIN_ROOT}` against the
    // installed root, and keys entries with the plugin pseudo-id. Approval
    // comes only from the trust record (#2942 S3.2), so both manifests carry
    // the scrubbed (false) flag like real session installs do.
    #[tokio::test]
    async fn session_plugin_configs_are_approved_only_and_root_resolved() {
        let store = tempfile::tempdir().unwrap();
        let trust_root = tempfile::tempdir().unwrap();
        let trust_dir = trust_root.path().join("sess-1");
        let approved_root = write_store_manifest(
            store.path(),
            "plugin-aaaa000000000000",
            false,
            r#"[{"name":"fetch","entry":{"command":"${CLAUDE_PLUGIN_ROOT}/bin/fetch","args":["--root","${CLAUDE_PLUGIN_ROOT}"]}}]"#,
        );
        write_store_manifest(
            store.path(),
            "plugin-bbbb000000000000",
            false,
            r#"[{"name":"secret","entry":{"command":"never-run"}}]"#,
        );
        add_store_entry(store.path(), "plugin-aaaa000000000000");
        add_store_entry(store.path(), "plugin-bbbb000000000000");
        record_mcp_trust(&trust_dir, "plugin-aaaa000000000000", true);

        let mut taken = std::collections::HashSet::new();
        let configs = build_session_plugin_server_configs(
            store.path(),
            "sess-1",
            Some(&trust_dir),
            &mut taken,
        )
        .await;
        assert_eq!(configs.len(), 1, "unapproved source servers are skipped");
        let server = &configs[0];
        assert_eq!(server.server_name, "fetch");
        assert_eq!(server.pseudo_id, "plugin:plugin-aaaa000000000000:fetch");
        let root = approved_root.to_string_lossy();
        match &server.config.transport {
            crate::mcp::types::TransportConfig::Stdio { command, args, .. } => {
                assert_eq!(command, &format!("{root}/bin/fetch"));
                assert_eq!(args, &vec![format!("--root"), root.to_string()]);
            }
            _ => panic!("expected stdio transport"),
        }
        // The store shape keeps the `pluginSource` marker (covered by
        // `entry_converts_to_stored_config_json`); the in-memory session
        // config identifies its owner through the pseudo-id instead.
        assert!(server
            .pseudo_id
            .starts_with("plugin:plugin-aaaa000000000000:"));
        // The session key is claimed so DB-loaded servers cannot collide.
        assert!(taken.contains("fetch"));
    }

    /// #2942 S3.2 fixture: list a plugin in the store manifest (session
    /// load is manifest-driven, #2942 D1) so the loader can see it.
    fn add_store_entry(base: &Path, source_id: &str) {
        crate::services::hooks::registry::upsert_session_store_entry(
            base,
            crate::services::hooks::registry::SessionPluginStoreEntry {
                source_id: source_id.to_string(),
                source_url: format!("https://example.com/{source_id}"),
                installed_at: 0,
            },
        )
        .unwrap();
    }

    /// #2942 S3.2 fixture: write the out-of-workspace trust record that is
    /// the only authority for MCP-server approval.
    fn record_mcp_trust(trust_dir: &Path, source_id: &str, approved: bool) {
        crate::services::hooks::registry::record_session_plugin_trust(
            trust_dir,
            source_id,
            &crate::services::hooks::registry::SessionPluginTrust {
                mcp_approved: approved,
                ..Default::default()
            },
        )
        .unwrap();
    }

    /// #2927 fixture: flip the store manifest's `enabled` flag the way
    /// `/plugin disable` does, without re-running an install.
    fn disable_store_source(base: &Path, source_id: &str) {
        let path = base.join(source_id).join("manifest.json");
        let raw = fs::read_to_string(&path).unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        value["enabled"] = serde_json::Value::Bool(false);
        fs::write(&path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
    }

    /// #2927: a disabled session plugin contributes no MCP servers even when
    /// its approval record still says yes (D4: the state flag wins, the
    /// approval and the DB rows stay for a later re-enable).
    #[test]
    fn disabled_plugin_contributes_no_mcp_servers() {
        let store = tempfile::tempdir().unwrap();
        let trust_root = tempfile::tempdir().unwrap();
        let trust_dir = trust_root.path().join("sess-7");
        let enabled_id = "plugin-aaaa000000000000";
        let disabled_id = "plugin-bbbb000000000000";
        write_store_manifest(
            store.path(),
            enabled_id,
            false,
            r#"[{"name":"enabled","entry":{"command":"bin/enabled"}}]"#,
        );
        write_store_manifest(
            store.path(),
            disabled_id,
            false,
            r#"[{"name":"disabled","entry":{"command":"bin/disabled"}}]"#,
        );
        add_store_entry(store.path(), enabled_id);
        add_store_entry(store.path(), disabled_id);
        record_mcp_trust(&trust_dir, enabled_id, true);
        record_mcp_trust(&trust_dir, disabled_id, true);
        disable_store_source(store.path(), disabled_id);

        let (servers, roots) = session_plugin_mcp_records(store.path(), Some(&trust_dir));
        let names: Vec<&str> = servers
            .iter()
            .map(|(_, snapshot)| snapshot.name.as_str())
            .collect();
        assert_eq!(names, vec!["enabled"], "disabled plugin servers: {names:?}");
        assert_eq!(servers[0].0.manifest.source_id, enabled_id);
        assert!(
            roots.contains_key(disabled_id),
            "the disabled plugin still shadows the global copy of its source_id"
        );
    }

    // #2942 S3.2: the session store now lives in the agent-writable
    // workspace, so a tampered workspace manifest claiming mcp_approved=true
    // must never start a server — only the out-of-workspace trust record
    // can approve.
    #[tokio::test]
    async fn tampered_session_manifest_cannot_start_plugin_mcp_servers() {
        let store = tempfile::tempdir().unwrap();
        let trust_root = tempfile::tempdir().unwrap();
        let trust_dir = trust_root.path().join("sess-9");
        let id = "plugin-cccc000000000000";
        write_store_manifest(
            store.path(),
            id,
            true, // tampered: the agent flipped the workspace flag
            r#"[{"name":"evil","entry":{"command":"never-run"}}]"#,
        );
        add_store_entry(store.path(), id);

        let mut taken = std::collections::HashSet::new();
        // No trust record: tampering alone starts nothing, both with the
        // real (empty) trust dir and with none at all.
        let configs = build_session_plugin_server_configs(
            store.path(),
            "sess-9",
            Some(&trust_dir),
            &mut taken,
        )
        .await;
        assert!(
            configs.is_empty(),
            "workspace tampering must not approve servers"
        );
        let configs =
            build_session_plugin_server_configs(store.path(), "sess-9", None, &mut taken).await;
        assert!(configs.is_empty());

        // Genuine approval through the trust record starts the server.
        record_mcp_trust(&trust_dir, id, true);
        let configs = build_session_plugin_server_configs(
            store.path(),
            "sess-9",
            Some(&trust_dir),
            &mut taken,
        )
        .await;
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].server_name, "evil");
        assert_eq!(configs[0].pseudo_id, "plugin:plugin-cccc000000000000:evil");
    }

    // #2926 gatekeeper G1.1: a session's option sidecar is read from the
    // out-of-workspace trust directory, never from the agent-writable
    // workspace plugin store — a sidecar the session's agent writes there
    // must never reach `CLAUDE_PLUGIN_OPTION_*` or `${user_config.*}`.
    #[tokio::test]
    async fn session_plugin_options_come_from_the_trust_dir_not_the_workspace() {
        let store = tempfile::tempdir().unwrap();
        let trust_root = tempfile::tempdir().unwrap();
        let trust_dir = trust_root.path().join("sess-7");
        let id = "plugin-dddd000000000000";
        write_store_manifest(
            store.path(),
            id,
            false,
            r#"[{"name":"srv","entry":{"command":"bin","args":["${user_config.TOKEN}"]}}]"#,
        );
        add_store_entry(store.path(), id);
        record_mcp_trust(&trust_dir, id, true);
        // The genuine values, exactly as a session install leaves them: the
        // sidecar sits in the trust directory.
        std::fs::create_dir_all(&trust_dir).unwrap();
        std::fs::write(
            trust_dir.join(format!("{id}.options.json")),
            r#"{ "values": { "TOKEN": "genuine" }, "sensitiveKeys": [] }"#,
        )
        .unwrap();
        // Tampering attempt: an extra sidecar in the agent-writable store.
        std::fs::write(
            store.path().join(format!("{id}.options.json")),
            r#"{ "values": { "TOKEN": "evil" }, "sensitiveKeys": [] }"#,
        )
        .unwrap();

        let mut taken = std::collections::HashSet::new();
        let configs = build_session_plugin_server_configs(
            store.path(),
            "sess-7",
            Some(&trust_dir),
            &mut taken,
        )
        .await;
        assert_eq!(configs.len(), 1);
        match &configs[0].config.transport {
            crate::mcp::types::TransportConfig::Stdio { args, env, .. } => {
                assert_eq!(
                    args,
                    &vec!["genuine".to_string()],
                    "the workspace sidecar must not expand into the server config"
                );
                assert_eq!(
                    env.get("CLAUDE_PLUGIN_OPTION_TOKEN").map(String::as_str),
                    Some("genuine"),
                    "CLAUDE_PLUGIN_OPTION_* must carry the trust sidecar's value"
                );
                assert!(
                    !env.values().any(|value| value == "evil"),
                    "a tampered workspace sidecar value must never be exported"
                );
            }
            _ => panic!("expected stdio transport"),
        }
    }

    async fn marker_count(db: &sea_orm::DatabaseConnection, marker: &str) -> usize {
        use sea_orm::EntityTrait;
        crate::entity::mcp_server::Entity::find()
            .all(db)
            .await
            .unwrap()
            .into_iter()
            .filter(|row| {
                crate::mcp::types::plugin_source_of(&row.config).as_deref() == Some(marker)
            })
            .count()
    }

    // #2924 (gatekeeper G1.1): re-install / update converges the global store
    // exactly on the approved snapshot — create never self-removes, and
    // remove deletes every owned row first.
    #[tokio::test]
    async fn create_and_remove_rows_converge_on_the_approved_snapshot() {
        use sea_orm_migration::MigratorTrait;
        let _guard = crate::state::lock_test_global_state().await;
        crate::lifecycle::database::register_sqlite_vec();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        crate::migration::Migrator::up(&db, None)
            .await
            .expect("migrations");
        crate::state::set_mcp_server_repository(
            crate::repositories::mcp_server_repository::SqliteMCPServerRepository::new(db.clone()),
        );

        let source_id = "plugin-cc00000000000001";
        let marker = plugin_source_marker(source_id);
        let root = std::path::Path::new("/tmp/plugin-root-under-test");
        let snapshot = |name: &str| PluginMcpServerSnapshot {
            name: name.to_string(),
            entry: serde_json::json!({"command": "true"}),
        };

        // First install: both approved servers become owned rows.
        let warnings =
            create_plugin_mcp_server_rows(source_id, root, &[snapshot("alpha"), snapshot("beta")])
                .await
                .expect("store write ok");
        assert!(warnings.is_empty());
        assert_eq!(marker_count(&db, &marker).await, 2);

        // Update that drops "beta": removal runs first, so only the new
        // snapshot survives (stale rows never keep running).
        remove_global_plugin_mcp_servers(source_id)
            .await
            .expect("removal ok");
        assert_eq!(marker_count(&db, &marker).await, 0);
        let warnings = create_plugin_mcp_server_rows(source_id, root, &[snapshot("alpha")])
            .await
            .expect("store write ok");
        assert!(warnings.is_empty());
        assert_eq!(marker_count(&db, &marker).await, 1);

        // Uninstall removes every owned row of the source.
        remove_global_plugin_mcp_servers(source_id)
            .await
            .expect("removal ok");
        assert_eq!(marker_count(&db, &marker).await, 0);
    }

    // #2924 (gatekeeper G2.1): the install-path re-sync itself (not just its
    // helpers) must remove every owned row BEFORE any early return — an
    // unapproved re-install, a dropped server, or an empty snapshot must
    // never leave stale rows in the global store.
    #[tokio::test]
    async fn re_sync_through_the_install_sync_path_converges() {
        use crate::services::hooks::registry::{PluginSourceManifest, PluginSourceRecord};
        use crate::services::skill_service::importing::sync_plugin_mcp_rows_from_record;
        use sea_orm_migration::MigratorTrait;
        let _guard = crate::state::lock_test_global_state().await;
        crate::lifecycle::database::register_sqlite_vec();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        crate::migration::Migrator::up(&db, None)
            .await
            .expect("migrations");
        crate::state::set_mcp_server_repository(
            crate::repositories::mcp_server_repository::SqliteMCPServerRepository::new(db.clone()),
        );

        let source_id = "plugin-cc00000000000003";
        let marker = plugin_source_marker(source_id);
        let snapshot = |name: &str| PluginMcpServerSnapshot {
            name: name.to_string(),
            entry: serde_json::json!({"command": "true"}),
        };
        let record = |approved: bool, servers: Vec<PluginMcpServerSnapshot>| PluginSourceRecord {
            manifest: PluginSourceManifest {
                source_url: "https://example.com/resync".to_string(),
                source_id: source_id.to_string(),
                skill_names: vec![],
                hooks: None,
                hooks_approved: false,
                approval_hashes: Default::default(),
                created_at: 0,
                plugin: None,
                hook_files: vec![],
                mcp_servers: servers,
                mcp_approved: approved,
                enabled: true,
                installed_commit: None,
            },
            root_dir: std::path::PathBuf::from("/tmp/plugin-root-under-test"),
        };

        // (1) Approved install with two servers → two owned rows.
        sync_plugin_mcp_rows_from_record(&record(true, vec![snapshot("alpha"), snapshot("beta")]))
            .await
            .expect("sync ok");
        assert_eq!(marker_count(&db, &marker).await, 2);

        // (2) Re-sync without approval → every stale row is gone, none created.
        sync_plugin_mcp_rows_from_record(&record(false, vec![snapshot("alpha"), snapshot("beta")]))
            .await
            .expect("sync ok");
        assert_eq!(marker_count(&db, &marker).await, 0);

        // Re-approve to seed the rows again, then (3) re-sync with "beta"
        // dropped → exactly the new snapshot survives.
        sync_plugin_mcp_rows_from_record(&record(true, vec![snapshot("alpha"), snapshot("beta")]))
            .await
            .expect("sync ok");
        assert_eq!(marker_count(&db, &marker).await, 2);
        sync_plugin_mcp_rows_from_record(&record(true, vec![snapshot("alpha")]))
            .await
            .expect("sync ok");
        assert_eq!(marker_count(&db, &marker).await, 1);

        // (4) Re-sync with an empty snapshot → zero rows.
        sync_plugin_mcp_rows_from_record(&record(true, vec![]))
            .await
            .expect("sync ok");
        assert_eq!(marker_count(&db, &marker).await, 0);
    }

    // #2924 (gatekeeper G1.2): an approved server that cannot be installed
    // must come back as a reported warning — never a silent drop.
    #[tokio::test]
    async fn uninstallable_server_reports_a_warning() {
        use sea_orm_migration::MigratorTrait;
        let _guard = crate::state::lock_test_global_state().await;
        crate::lifecycle::database::register_sqlite_vec();
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("memory db");
        crate::migration::Migrator::up(&db, None)
            .await
            .expect("migrations");
        crate::state::set_mcp_server_repository(
            crate::repositories::mcp_server_repository::SqliteMCPServerRepository::new(db),
        );

        let root = std::path::Path::new("/tmp/plugin-root-under-test");
        let good = PluginMcpServerSnapshot {
            name: "good".to_string(),
            entry: serde_json::json!({"command": "true"}),
        };
        let broken = PluginMcpServerSnapshot {
            name: "broken".to_string(),
            entry: serde_json::json!({"noTransport": true}),
        };
        let warnings =
            create_plugin_mcp_server_rows("plugin-cc00000000000002", root, &[good, broken])
                .await
                .expect("the store itself stays healthy; only the row fails");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("broken"), "{warnings:?}");
        assert!(
            warnings[0].contains("could not be installed"),
            "{warnings:?}"
        );
    }
}
