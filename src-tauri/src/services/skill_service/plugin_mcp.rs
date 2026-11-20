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

/// Create the plugin-owned rows for `servers` in the global MCP server
/// store. Callers must remove any rows previously owned by `source_id`
/// first (see [`remove_global_plugin_mcp_servers`]) so re-install and
/// update converge exactly on `servers` (gatekeeper G1.1).
///
/// Returns one warning per server that could not be installed; the caller
/// must surface them to the user — the user approved these servers, so a
/// silent drop is not acceptable (gatekeeper G1.2). A failure to read or
/// write the store itself is an `Err`.
pub async fn create_plugin_mcp_server_rows(
    source_id: &str,
    plugin_root: &Path,
    servers: &[PluginMcpServerSnapshot],
) -> Result<Vec<String>, String> {
    use crate::services::{McpServerService, SaveVerification};
    let repo = crate::state::get_mcp_server_repository();

    let mut warnings = Vec::new();
    for snapshot in servers {
        let name = sanitize_mcp_server_name(&snapshot.name);
        let outcome: Result<String, String> = async {
            let config = snapshot_to_mcp_config_json(snapshot, plugin_root, source_id)?;
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
/// session's servers, same as DB-loaded servers.
pub fn build_session_plugin_server_configs(
    session_store: &Path,
    session_id: &str,
    taken_session_names: &mut HashSet<String>,
) -> Vec<SessionPluginServerConfig> {
    use crate::mcp::types::MCPServerConfig;
    use crate::mcp::utils::unique_session_server_name;

    let (servers, roots) = session_plugin_mcp_servers(session_store);
    let mut out = Vec::new();
    for (source_id, snapshot) in servers {
        let Some(root) = roots.get(&source_id) else {
            continue;
        };
        let config_json = match snapshot_to_mcp_config_json(&snapshot, root, &source_id) {
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
                  "mcp_approved": {mcp_approved}
                }}"#
            ),
        )
        .unwrap();
        root
    }

    // #2924 (gatekeeper G1.4): session load reads only approved servers from
    // the session store, resolves `${CLAUDE_PLUGIN_ROOT}` against the
    // installed root, and keys entries with the plugin pseudo-id.
    #[test]
    fn session_plugin_configs_are_approved_only_and_root_resolved() {
        let store = tempfile::tempdir().unwrap();
        let approved_root = write_store_manifest(
            store.path(),
            "plugin-aaaa000000000000",
            true,
            r#"[{"name":"fetch","entry":{"command":"${CLAUDE_PLUGIN_ROOT}/bin/fetch","args":["--root","${CLAUDE_PLUGIN_ROOT}"]}}]"#,
        );
        write_store_manifest(
            store.path(),
            "plugin-bbbb000000000000",
            false,
            r#"[{"name":"secret","entry":{"command":"never-run"}}]"#,
        );

        let mut taken = std::collections::HashSet::new();
        let configs = build_session_plugin_server_configs(store.path(), "sess-1", &mut taken);
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
