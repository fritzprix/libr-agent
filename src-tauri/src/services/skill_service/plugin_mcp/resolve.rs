use super::PluginExpansion;
use serde_json::Value;
#[cfg(test)]
use std::path::Path;

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
