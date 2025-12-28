use serde::{Deserialize, Serialize};

/// Agent configuration defining the AI agent's behavior and capabilities
/// This structure matches the TypeScript Assistant interface from src/models/chat.ts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig {
    /// Assistant ID (optional, generated if not provided)
    pub id: Option<String>,

    /// Assistant name
    pub name: String,

    /// Assistant description
    pub description: Option<String>,

    /// System prompt defining the agent's role and behavior
    pub system_prompt: String,

    /// MCP server IDs to connect to (references to MCPServerEntity IDs)
    #[serde(default)]
    pub mcp_server_ids: Vec<String>,

    /// Local services (legacy, may be deprecated)
    #[serde(default)]
    pub local_services: Vec<String>,

    /// Allowed built-in service aliases
    /// - None = all built-in services allowed (default)
    /// - Some([]) = no built-in services enabled
    /// - Some([...]) = specific services allowed
    pub allowed_built_in_service_aliases: Option<Vec<String>>,

    /// LLM model to use (from ModelChoice)
    #[serde(default = "default_model")]
    pub model: String,

    /// LLM provider (from ModelChoice)
    #[serde(default = "default_provider")]
    pub provider: String,

    /// Temperature for LLM (0.0-2.0, default 1.0)
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
}

fn default_temperature() -> f32 {
    1.0
}

fn default_model() -> String {
    "gpt-4".to_string()
}

fn default_provider() -> String {
    "openai".to_string()
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            id: None,
            name: "Default Assistant".to_string(),
            description: None,
            system_prompt: "You are a helpful AI assistant.".to_string(),
            mcp_server_ids: Vec::new(),
            local_services: Vec::new(),
            allowed_built_in_service_aliases: None, // Allow all by default
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            temperature: 1.0,
            max_tokens: None,
        }
    }
}

impl AgentConfig {
    /// Parse agent config from JSON string
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Failed to parse agent config: {}", e))
    }

    /// Serialize agent config to JSON string
    #[allow(dead_code)]
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| format!("Failed to serialize agent config: {}", e))
    }

    /// Validate agent configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("Assistant name cannot be empty".to_string());
        }

        if self.system_prompt.is_empty() {
            return Err("System prompt cannot be empty".to_string());
        }

        if self.model.is_empty() {
            return Err("Model name cannot be empty".to_string());
        }

        if self.provider.is_empty() {
            return Err("Provider name cannot be empty".to_string());
        }

        if self.temperature < 0.0 || self.temperature > 2.0 {
            return Err("Temperature must be between 0.0 and 2.0".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AgentConfig::default();
        assert_eq!(config.model, "gpt-4");
        assert_eq!(config.provider, "openai");
        assert_eq!(config.temperature, 1.0);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_json_serialization() {
        let config = AgentConfig {
            id: Some("assistant-1".to_string()),
            name: "Test Assistant".to_string(),
            description: Some("A test assistant".to_string()),
            system_prompt: "You are a helpful assistant".to_string(),
            mcp_server_ids: vec!["server1".to_string()],
            local_services: vec![],
            allowed_built_in_service_aliases: Some(vec!["browser".to_string()]),
            model: "claude-3-5-sonnet-20241022".to_string(),
            provider: "anthropic".to_string(),
            temperature: 0.7,
            max_tokens: Some(4096),
        };

        let json = config.to_json().unwrap();
        let parsed = AgentConfig::from_json(&json).unwrap();

        assert_eq!(parsed.model, config.model);
        assert_eq!(parsed.provider, config.provider);
        assert_eq!(parsed.temperature, config.temperature);
        assert_eq!(parsed.name, config.name);
    }

    #[test]
    fn test_validation() {
        let mut config = AgentConfig::default();

        // Valid config
        assert!(config.validate().is_ok());

        // Invalid temperature
        config.temperature = 3.0;
        assert!(config.validate().is_err());

        config.temperature = -0.5;
        assert!(config.validate().is_err());

        // Invalid model
        config.temperature = 1.0;
        config.model = String::new();
        assert!(config.validate().is_err());

        // Invalid name
        config.model = "gpt-4".to_string();
        config.name = String::new();
        assert!(config.validate().is_err());
    }
}
