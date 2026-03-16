use crate::entity::assistant::Model as AssistantModel;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantConfig {
    pub description: Option<String>,
    pub avatar: Option<String>,
    pub system_prompt: String,
    pub mcp_server_ids: Option<Vec<String>>,
    pub local_services: Option<Vec<String>>,
    pub allowed_built_in_service_aliases: Option<Vec<String>>,
    pub deletion_protected: bool,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub temperature: Option<f64>,
    pub disabled_skills: Option<Vec<String>>,
    pub max_tokens: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantDto {
    pub id: String,
    pub name: String,
    pub config: Value, // JSON config
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<AssistantModel> for AssistantDto {
    fn from(model: AssistantModel) -> Self {
        Self {
            id: model.id,
            name: model.name,
            config: serde_json::from_str(&model.config).unwrap_or(Value::Null),
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}
