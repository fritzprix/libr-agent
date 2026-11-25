use super::super::plugin_manifest::{validate_component_path, McpServersDecl, PluginManifest};
use super::PLUGIN_MCP_FILE;
use crate::services::hooks::registry::PluginMcpServerSnapshot;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::Path;

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
