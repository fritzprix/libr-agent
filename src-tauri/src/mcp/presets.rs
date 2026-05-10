use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerPreset {
    pub name: String,
    pub category: Option<String>,
    pub description: Option<String>,
    pub logo: Option<String>,
    pub transport_type: String, // "stdio" or "sse"
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<Value>,
    pub variable_definitions: Option<Value>,
    pub url: Option<String>,
}

#[derive(Deserialize)]
struct RawPresetConfig {
    category: Option<String>,
    // stdio fields
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    env: Option<Value>,
    #[serde(default, rename = "variableDefinitions")]
    variable_definitions: Option<Value>,
    // http fields
    url: Option<String>,
    // common
    description: Option<String>,
    logo: Option<String>,
}

pub fn get_recommended_servers() -> Vec<MCPServerPreset> {
    let json_content = include_str!("../../../mcp-server.json");

    #[derive(Deserialize)]
    struct RawPresetsMap {
        #[serde(rename = "mcpServers")]
        mcp_servers: HashMap<String, Value>,
    }

    let raw: RawPresetsMap = match serde_json::from_str(json_content) {
        Ok(parsed) => parsed,
        Err(e) => {
            log::error!("Failed to parse embedded mcp-server.json: {}", e);
            return vec![];
        }
    };

    let mut presets = Vec::new();

    for (key, value) in raw.mcp_servers {
        let config: RawPresetConfig = match serde_json::from_value(value) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Skipping invalid preset '{}': {}", key, e);
                continue;
            }
        };

        let (transport_type, command, args, url) = if config.url.is_some() {
            ("sse".to_string(), None, None, config.url)
        } else {
            ("stdio".to_string(), config.command, Some(config.args), None)
        };

        // Merge headers into variableDefinitions for HTTP servers if needed
        let variable_definitions = config.variable_definitions;

        presets.push(MCPServerPreset {
            name: key,
            category: config.category,
            description: config.description,
            logo: config.logo,
            transport_type,
            command,
            args,
            env: config.env,
            variable_definitions,
            url,
        });
    }

    // Sort by name for consistent display
    presets.sort_by(|a, b| a.name.cmp(&b.name));

    presets
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: HTTP-only presets (no command) must not be silently skipped.
    #[test]
    fn test_http_preset_is_parsed() {
        let presets = get_recommended_servers();
        // exa is an HTTP preset with url-param auth - must be present
        let exa = presets.iter().find(|p| p.name == "exa");
        assert!(exa.is_some(), "exa preset must be present");
        let exa = exa.unwrap();
        assert_eq!(exa.transport_type, "sse");
        assert!(exa.url.is_some(), "exa must have a url");
        assert!(exa.command.is_none(), "exa must not have a command");
        assert_eq!(exa.category.as_deref(), Some("search"));
    }

    /// Regression: stdlib stdio presets must still parse correctly after making command Optional.
    #[test]
    fn test_stdio_preset_is_parsed() {
        let presets = get_recommended_servers();
        let ddg = presets.iter().find(|p| p.name == "ddg-search");
        assert!(ddg.is_some(), "ddg-search preset must be present");
        let ddg = ddg.unwrap();
        assert_eq!(ddg.transport_type, "stdio");
        assert!(ddg.command.is_some(), "ddg-search must have a command");
        assert!(ddg.url.is_none());
        assert_eq!(ddg.category.as_deref(), Some("search"));
    }

    /// All presets in mcp-server.json must parse without panicking.
    #[test]
    fn test_all_presets_parse() {
        let presets = get_recommended_servers();
        assert!(!presets.is_empty(), "at least one preset must exist");
        for p in &presets {
            assert!(!p.name.is_empty(), "preset name must not be empty");
            assert!(
                p.transport_type == "stdio" || p.transport_type == "sse",
                "transport_type must be stdio or sse, got: {}",
                p.transport_type
            );
            assert!(
                p.category.as_deref().is_some(),
                "preset {} must declare a category",
                p.name
            );
        }
    }
}
