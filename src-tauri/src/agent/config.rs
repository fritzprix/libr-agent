use crate::mcp::builtin::service_id::BuiltinServiceId;
use serde::{Deserialize, Serialize};

/// Agent configuration defining the AI agent's behavior and capabilities
/// This structure matches the TypeScript Assistant interface from src/models/chat.ts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig {
    /// Assistant ID (optional, generated if not provided)
    #[serde(alias = "assistantId", alias = "assistant_id")]
    pub id: Option<String>,

    /// Assistant name
    #[serde(default = "default_name")]
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
    pub allowed_built_in_service_aliases: Option<Vec<BuiltinServiceId>>,

    // NOTE: Model and Provider have been moved to the session level or global settings.
    // Assistants focus on identity (system prompt) and capabilities (MCP servers).
    // The actual LLM configuration follows global settings or session overrides.
    /// Temperature for LLM (0.0-2.0, default 1.0)
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,

    /// Optional maximum recursive child depth (None = unlimited)
    pub max_depth: Option<u32>,

    /// Optional maximum direct children per parent session (None = unlimited)
    pub max_fanout: Option<u32>,

    /// Optional parent session ID for nested session lineage
    pub parent_session_id: Option<String>,

    /// Optional lineage root identifier shared across a session tree
    pub lineage_id: Option<String>,

    /// Optional hierarchy depth (root=0)
    pub depth: Option<u32>,
}

fn default_temperature() -> f32 {
    1.0
}

fn default_name() -> String {
    "Unknown Assistant".to_string()
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
            temperature: 1.0,
            max_tokens: None,
            max_depth: None,
            max_fanout: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
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
            allowed_built_in_service_aliases: Some(vec![BuiltinServiceId::Browser]),
            temperature: 0.7,
            max_tokens: Some(4096),
            max_depth: Some(8),
            max_fanout: Some(4),
            parent_session_id: Some("session-parent".to_string()),
            lineage_id: Some("lineage-root".to_string()),
            depth: Some(1),
        };

        let json = config.to_json().unwrap();
        let parsed = AgentConfig::from_json(&json).unwrap();

        assert_eq!(parsed.id, config.id);
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

        // Invalid name
        config.temperature = 1.0;
        config.name = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_id_alias_deserialization() {
        let camel_case_json = r#"{
            "assistantId": "assistant-camel",
            "name": "Alias Test",
            "systemPrompt": "You are helpful"
        }"#;

        let snake_case_json = r#"{
            "assistant_id": "assistant-snake",
            "name": "Alias Test",
            "systemPrompt": "You are helpful"
        }"#;

        let parsed_camel = AgentConfig::from_json(camel_case_json).unwrap();
        let parsed_snake = AgentConfig::from_json(snake_case_json).unwrap();

        assert_eq!(parsed_camel.id.as_deref(), Some("assistant-camel"));
        assert_eq!(parsed_snake.id.as_deref(), Some("assistant-snake"));
    }

    /// Regression: legacy "content_store" in DB JSON must deserialise to Attachments.
    #[test]
    fn test_allowed_aliases_legacy_content_store_deserializes() {
        let json = r#"{
            "name": "Legacy Assistant",
            "systemPrompt": "You are helpful",
            "allowedBuiltInServiceAliases": ["content_store", "browser"]
        }"#;
        let config = AgentConfig::from_json(json).unwrap();
        let aliases = config.allowed_built_in_service_aliases.unwrap();
        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0], BuiltinServiceId::Attachments);
        assert_eq!(aliases[1], BuiltinServiceId::Browser);
    }

    /// Canonical "attachments" name must also deserialise correctly.
    #[test]
    fn test_allowed_aliases_canonical_names_deserialize() {
        let json = r#"{
            "name": "Test",
            "systemPrompt": "You are helpful",
            "allowedBuiltInServiceAliases": ["attachments", "planning", "browser"]
        }"#;
        let config = AgentConfig::from_json(json).unwrap();
        let aliases = config.allowed_built_in_service_aliases.unwrap();
        assert_eq!(aliases[0], BuiltinServiceId::Attachments);
        assert_eq!(aliases[1], BuiltinServiceId::Planning);
        assert_eq!(aliases[2], BuiltinServiceId::Browser);
    }

    /// Unknown alias string must cause a parse error (compile-time safety).
    #[test]
    fn test_allowed_aliases_unknown_string_errors() {
        let json = r#"{
            "name": "Test",
            "systemPrompt": "You are helpful",
            "allowedBuiltInServiceAliases": ["not_a_real_service"]
        }"#;
        assert!(AgentConfig::from_json(json).is_err());
    }

    /// Serialise then deserialise must be identity.
    #[test]
    fn test_allowed_aliases_roundtrip() {
        let config = AgentConfig {
            allowed_built_in_service_aliases: Some(vec![
                BuiltinServiceId::Attachments,
                BuiltinServiceId::Browser,
            ]),
            ..AgentConfig::default()
        };
        let json = config.to_json().unwrap();
        let parsed = AgentConfig::from_json(&json).unwrap();
        assert_eq!(
            parsed.allowed_built_in_service_aliases,
            config.allowed_built_in_service_aliases
        );
    }
}
