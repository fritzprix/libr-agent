use crate::entity::assistant::Model as AssistantModel;
use crate::repositories::AssistantRepository;
use crate::state::get_assistant_repository;
use serde_json::Value;

pub struct AssistantService;

impl AssistantService {
    pub async fn create_assistant(
        id: String,
        name: String,
        config: Value,
    ) -> Result<AssistantModel, String> {
        let repo = get_assistant_repository();
        let config_str = config.to_string();

        repo.create_assistant(id, name, config_str)
            .await
            .map_err(|e| format!("Failed to create assistant: {}", e))
    }

    pub async fn update_assistant(
        id: &str,
        name: Option<String>,
        config: Option<Value>,
    ) -> Result<AssistantModel, String> {
        let repo = get_assistant_repository();
        let config_str = config.map(|c| c.to_string());

        repo.update_assistant(id, name, config_str)
            .await
            .map_err(|e| format!("Failed to update assistant: {}", e))
    }

    pub async fn delete_assistant(id: &str) -> Result<(), String> {
        let repo = get_assistant_repository();
        repo.delete_assistant(id)
            .await
            .map_err(|e| format!("Failed to delete assistant: {}", e))
    }

    pub async fn list_assistants() -> Result<Vec<AssistantModel>, String> {
        let repo = get_assistant_repository();
        repo.list_assistants()
            .await
            .map_err(|e| format!("Failed to list assistants: {}", e))
    }

    pub async fn get_assistant(id: &str) -> Result<Option<AssistantModel>, String> {
        let repo = get_assistant_repository();
        repo.get_assistant(id)
            .await
            .map_err(|e| format!("Failed to get assistant: {}", e))
    }

    pub async fn batch_upsert_assistants(
        assistants: Vec<crate::commands::assistant_crud_commands::UpsertAssistantPayload>,
    ) -> Result<Vec<AssistantModel>, String> {
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

            results.push(result);
        }

        Ok(results)
    }
}
