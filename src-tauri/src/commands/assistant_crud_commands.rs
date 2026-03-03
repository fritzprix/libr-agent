use crate::entity::assistant::Model as AssistantModel;
use crate::repositories::AssistantRepository;
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
    let repo = get_assistant_repository();
    let config_str = config.to_string();

    let result = repo
        .create_assistant(id, name, config_str)
        .await
        .map_err(|e| format!("Failed to create assistant: {}", e))?;

    Ok(result.into())
}

#[command]
pub async fn update_assistant(
    id: String,
    name: Option<String>,
    config: Option<Value>,
) -> Result<AssistantDto, String> {
    let repo = get_assistant_repository();
    let config_str = config.map(|c| c.to_string());

    let result = repo
        .update_assistant(&id, name, config_str)
        .await
        .map_err(|e| format!("Failed to update assistant: {}", e))?;

    Ok(result.into())
}

#[command]
pub async fn delete_assistant(id: String) -> Result<(), String> {
    let repo = get_assistant_repository();
    repo.delete_assistant(&id)
        .await
        .map_err(|e| format!("Failed to delete assistant: {}", e))?;
    Ok(())
}

#[command]
pub async fn list_assistants() -> Result<Vec<AssistantDto>, String> {
    let repo = get_assistant_repository();
    let assistants = repo
        .list_assistants()
        .await
        .map_err(|e| format!("Failed to list assistants: {}", e))?;

    Ok(assistants.into_iter().map(|a| a.into()).collect())
}

#[command]
pub async fn get_assistant(id: String) -> Result<Option<AssistantDto>, String> {
    let repo = get_assistant_repository();
    let assistant = repo
        .get_assistant(&id)
        .await
        .map_err(|e| format!("Failed to get assistant: {}", e))?;

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
    let repo = get_assistant_repository();
    let mut results = Vec::new();

    for payload in assistants {
        let config_str = payload.config.to_string();

        let update_result = repo
            .update_assistant(
                &payload.id,
                Some(payload.name.clone()),
                Some(config_str.clone()),
            )
            .await;

        let result = match update_result {
            Ok(model) => model,
            Err(crate::repositories::DbError::NotFound(_)) => repo
                .create_assistant(payload.id.clone(), payload.name, config_str)
                .await
                .map_err(|e| format!("Failed to create assistant {}: {}", payload.id, e))?,
            Err(e) => {
                return Err(format!("Failed to update assistant {}: {}", payload.id, e));
            }
        };

        results.push(result.into());
    }

    Ok(results)
}
