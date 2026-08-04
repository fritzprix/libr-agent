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

    /// Allowed built-in service aliases
    /// - None = all built-in services allowed (default)
    /// - Some([]) = no built-in services enabled
    /// - Some([...]) = specific services allowed
    pub allowed_built_in_service_aliases: Option<Vec<BuiltinServiceId>>,

    // NOTE: Model and provider live at session / global settings. Sampling params such as
    // temperature are omitted so provider/serving-engine defaults apply. Assistants focus
    // on identity (system prompt) and capabilities (MCP servers).
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

    /// Optional explicit org identity for org-only teamwork lineages
    pub org_id: Option<String>,

    /// Optional display name for the explicit org identity
    pub org_name: Option<String>,

    /// Optional root session ID that should be resumed from org UX
    pub org_root_session_id: Option<String>,
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
            allowed_built_in_service_aliases: None, // Allow all by default
            max_tokens: None,
            max_depth: None,
            max_fanout: None,
            parent_session_id: None,
            lineage_id: None,
            depth: None,
            org_id: None,
            org_name: None,
            org_root_session_id: None,
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

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AgentConfig::default();
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
            allowed_built_in_service_aliases: Some(vec![BuiltinServiceId::Browser]),
            max_tokens: Some(8192),
            max_depth: Some(8),
            max_fanout: Some(4),
            parent_session_id: Some("session-parent".to_string()),
            lineage_id: Some("lineage-root".to_string()),
            depth: Some(1),
            org_id: Some("org-1".to_string()),
            org_name: Some("Org One".to_string()),
            org_root_session_id: Some("session-root".to_string()),
        };

        let json = config.to_json().unwrap();
        let parsed = AgentConfig::from_json(&json).unwrap();

        assert_eq!(parsed.id, config.id);
        assert_eq!(parsed.max_tokens, config.max_tokens);
        assert_eq!(parsed.name, config.name);
    }

    #[test]
    fn test_validation() {
        let mut config = AgentConfig::default();

        // Valid config
        assert!(config.validate().is_ok());

        // Invalid name
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
