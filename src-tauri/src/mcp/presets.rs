use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerPreset {
    pub name: String,
    pub description: Option<String>,
    pub transport_type: String, // "stdio" or "sse"
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<Value>,
    pub variable_definitions: Option<Value>,
    pub url: Option<String>,
}

#[derive(Deserialize)]
struct RawPresetConfig {
    command: String,
    args: Vec<String>,
    env: Option<Value>,
    #[serde(default, rename = "variableDefinitions")]
    variable_definitions: Option<Value>,
    description: Option<String>,
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

        presets.push(MCPServerPreset {
            name: key,
            description: config.description,
            transport_type: "stdio".to_string(),
            command: Some(config.command),
            args: Some(config.args),
            env: config.env,
            variable_definitions: config.variable_definitions,
            url: None,
        });
    }

    // Sort by name for consistent display
    presets.sort_by(|a, b| a.name.cmp(&b.name));

    presets
}
