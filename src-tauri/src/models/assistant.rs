use crate::entity::assistant::Model as AssistantModel;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantSummaryDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub deletion_protected: bool,
}

impl From<AssistantModel> for AssistantSummaryDto {
    fn from(model: AssistantModel) -> Self {
        let config = serde_json::from_str::<Value>(&model.config).unwrap_or(Value::Null);

        Self {
            id: model.id,
            name: model.name,
            description: config
                .get("description")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string()),
            deletion_protected: config
                .get("deletionProtected")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
        }
    }
}
