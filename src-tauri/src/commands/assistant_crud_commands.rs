use crate::entity::assistant::Model as AssistantModel;
use crate::services::AssistantService;
use crate::state::get_assistant_repository;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::command;

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

#[command]
pub async fn create_assistant(
    id: String,
    name: String,
    config: Value,
) -> Result<AssistantDto, String> {
    let result =
        AssistantService::create_assistant(get_assistant_repository(), id, name, config).await?;
    Ok(result.into())
}

#[command]
pub async fn update_assistant(
    id: String,
    name: Option<String>,
    config: Option<Value>,
) -> Result<AssistantDto, String> {
    let result =
        AssistantService::update_assistant(get_assistant_repository(), &id, name, config).await?;
    Ok(result.into())
}

#[command]
pub async fn delete_assistant(id: String) -> Result<(), String> {
    AssistantService::delete_assistant(get_assistant_repository(), &id).await
}

#[command]
pub async fn list_assistants() -> Result<Vec<AssistantDto>, String> {
    let assistants = AssistantService::list_assistants(get_assistant_repository()).await?;
    Ok(assistants.into_iter().map(|a| a.into()).collect())
}

#[command]
pub async fn get_assistant(id: String) -> Result<Option<AssistantDto>, String> {
    let assistant = AssistantService::get_assistant(get_assistant_repository(), &id).await?;
    Ok(assistant.map(|a| a.into()))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertAssistantPayload {
    pub id: String,
    pub name: String,
    pub config: Value,
}

#[command]
pub async fn batch_upsert_assistants(
    assistants: Vec<UpsertAssistantPayload>,
) -> Result<Vec<AssistantDto>, String> {
    let service_payloads = assistants
        .into_iter()
        .map(
            |p| crate::services::assistant_service::AssistantUpsertPayload {
                id: p.id,
                name: p.name,
                config: p.config,
            },
        )
        .collect();

    let results =
        AssistantService::batch_upsert_assistants(get_assistant_repository(), service_payloads)
            .await?;
    Ok(results.into_iter().map(|a| a.into()).collect())
}
