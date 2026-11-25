use serde_json::Value;

/// The `pluginSource` config key, shared with the #2924 MCP-server marker.
pub const PLUGIN_SOURCE_KEY: &str = "pluginSource";
/// Every plugin-owned assistant id starts with this prefix (D2).
pub const PLUGIN_AGENT_ID_PREFIX: &str = "plugin:";

/// The deterministic assistant id for one plugin agent (D2).
pub fn plugin_agent_id(source_id: &str, agent_name: &str) -> String {
    format!("{PLUGIN_AGENT_ID_PREFIX}{source_id}:{agent_name}")
}

/// The `pluginSource` ownership marker value for a plugin source (D2).
pub fn plugin_source_marker(source_id: &str) -> String {
    format!("plugin:{source_id}")
}

/// True for every id this feature owns (`plugin:<source_id>:<agent>` or a
/// bare `plugin:<source_id>` — both are reserved for plugin flows).
pub fn is_plugin_agent_id(id: &str) -> bool {
    id.starts_with(PLUGIN_AGENT_ID_PREFIX)
}

/// Parse `plugin:<source_id>:<agent>` into its parts (`None` for any other
/// id shape). Source ids never contain `:` and agent names are sanitized
/// without `:`, so the first `:` after the prefix splits the two.
pub fn parse_plugin_agent_id(id: &str) -> Option<(&str, &str)> {
    let rest = id.strip_prefix(PLUGIN_AGENT_ID_PREFIX)?;
    let (source_id, agent) = rest.split_once(':')?;
    if source_id.is_empty() || agent.is_empty() {
        return None;
    }
    Some((source_id, agent))
}

/// Reject user mutations of plugin-owned assistant ids (D2). Called by
/// every assistant mutation path (Tauri commands, AssistantService, MCP
/// `agent__createAgent`/`agent__updateAgent`/`agent__deleteAgent`), and
/// again for any config that spoofs the ownership marker.
pub fn reject_plugin_agent_mutation(id: &str, new_config: Option<&Value>) -> Result<(), String> {
    if is_plugin_agent_id(id) {
        return Err(format!(
            "Assistant {id} is owned by a plugin and is read-only: it is updated \
             or removed only with its plugin."
        ));
    }
    if let Some(config) = new_config {
        if config.get(PLUGIN_SOURCE_KEY).is_some() {
            return Err(format!(
                "The `{PLUGIN_SOURCE_KEY}` config field is reserved for plugin installs \
                 and cannot be set on a user assistant."
            ));
        }
    }
    Ok(())
}

/// Read the `pluginSource` marker out of a stored assistant config string.
pub fn plugin_source_of_config(config_json: &str) -> Option<String> {
    let value: Value = serde_json::from_str(config_json).ok()?;
    value
        .get(PLUGIN_SOURCE_KEY)
        .and_then(Value::as_str)
        .map(str::to_string)
}
