use super::super::plugin_manifest;
#[cfg(test)]
use super::no_options;
use super::resolve::resolve_plugin_placeholders;
use super::PluginExpansion;
use crate::mcp::types::PLUGIN_SOURCE_KEY;
use crate::services::hooks::registry::PluginMcpServerSnapshot;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

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
            if let Some(env_name) = plugin_manifest::option_env_name(key) {
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
