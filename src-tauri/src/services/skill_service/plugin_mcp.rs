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

use super::plugin_manifest::{McpServersDecl, PluginManifest};
use crate::mcp::types::PLUGIN_SOURCE_KEY;
use crate::mcp::utils::sanitize_mcp_server_name;
use crate::services::hooks::registry::PluginMcpServerSnapshot;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
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
                    let resolved = scan_root.join(&raw);
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
pub fn resolve_plugin_root(entry: &Value, plugin_root: &Path) -> Value {
    let root = plugin_root.to_string_lossy();
    resolve_value(entry, &root)
}

fn resolve_value(value: &Value, root: &str) -> Value {
    match value {
        Value::String(text) => {
            let mut expanded = text.clone();
            for placeholder in PLUGIN_ROOT_PLACEHOLDERS {
                if expanded.contains(placeholder) {
                    expanded = expanded.replace(placeholder, root);
                }
            }
            Value::String(expanded)
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| resolve_value(item, root)).collect())
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, item)| (key.clone(), resolve_value(item, root)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Convert one resolved plugin entry into the `MCPServerConfig` JSON shape
/// the global MCP server store keeps (`{"name", "transport": {...}}`).
pub fn entry_to_mcp_config_json(
    name: &str,
    resolved_entry: &Value,
    source_id: &str,
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
        let env = match obj.get("env") {
            Some(Value::Object(env)) => Value::Object(env.clone()),
            _ => Value::Object(Map::new()),
        };
        transport.insert("env".into(), env);
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

/// Build the config JSON for one snapshot against one installed root.
pub fn snapshot_to_mcp_config_json(
    snapshot: &PluginMcpServerSnapshot,
    plugin_root: &Path,
    source_id: &str,
) -> Result<Value, String> {
    let resolved = resolve_plugin_root(&snapshot.entry, plugin_root);
    entry_to_mcp_config_json(&snapshot.name, &resolved, source_id)
}

/// Install (or refresh) a global plugin's approved servers as plugin-owned
/// rows in the global MCP server store. Replaces every row previously owned
/// by `source_id`, so re-install and update stay idempotent.
pub async fn sync_global_plugin_mcp_servers(
    source_id: &str,
    plugin_root: &Path,
    servers: &[PluginMcpServerSnapshot],
) -> Result<(), String> {
    use crate::services::{McpServerService, SaveVerification};
    let repo = crate::state::get_mcp_server_repository();

    remove_global_plugin_mcp_servers(source_id).await?;

    for snapshot in servers {
        let config = snapshot_to_mcp_config_json(snapshot, plugin_root, source_id)?;
        let name = sanitize_mcp_server_name(&snapshot.name);
        match McpServerService::create_server_config(
            repo,
            name.clone(),
            config,
            SaveVerification::Pending,
        )
        .await
        {
            Ok(model) => {
                McpServerService::schedule_background_probe(model.id);
            }
            Err(error) => {
                log::warn!(
                    "Plugin {source_id}: could not install MCP server '{}' into the global store: {}",
                    snapshot.name,
                    error
                );
            }
        }
    }
    Ok(())
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
/// store). Returns `(source_id, snapshot)` pairs plus the set of source ids
/// present in the session store, which shadow same-id global plugins.
pub fn session_plugin_mcp_servers(
    session_store: &Path,
) -> (
    Vec<(String, PluginMcpServerSnapshot)>,
    BTreeMap<String, std::path::PathBuf>,
) {
    use crate::services::hooks::registry::list_sources_in;
    let records = list_sources_in(session_store).unwrap_or_else(|error| {
        log::warn!(
            "Failed to read session plugin store at {}: {error}",
            session_store.display()
        );
        Vec::new()
    });
    let mut out = Vec::new();
    let mut roots = BTreeMap::new();
    for record in records {
        let source_id = record.manifest.source_id.clone();
        roots.insert(source_id.clone(), record.root_dir);
        if !record.manifest.mcp_approved {
            continue;
        }
        for snapshot in record.manifest.mcp_servers.clone() {
            out.push((source_id.clone(), snapshot));
        }
    }
    (out, roots)
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
}
