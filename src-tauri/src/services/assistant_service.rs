use crate::entity::assistant::Model as AssistantModel;
use crate::repositories::AssistantRepository;
use serde_json::Value;

pub struct AssistantUpsertPayload {
    pub id: String,
    pub name: String,
    pub config: Value,
}

pub struct AssistantService;

impl AssistantService {
    pub async fn create_assistant(
        repo: &dyn AssistantRepository,
        id: String,
        name: String,
        config: Value,
    ) -> Result<AssistantModel, String> {
        let config_str = config.to_string();

        repo.create_assistant(id, name, config_str)
            .await
            .map_err(|e| format!("Failed to create assistant: {}", e))
    }

    pub async fn update_assistant(
        repo: &dyn AssistantRepository,
        id: &str,
        name: Option<String>,
        config: Option<Value>,
    ) -> Result<AssistantModel, String> {
        let config_str = config.map(|c| c.to_string());

        repo.update_assistant(id, name, config_str)
            .await
            .map_err(|e| format!("Failed to update assistant: {}", e))
    }

    pub async fn delete_assistant(repo: &dyn AssistantRepository, id: &str) -> Result<(), String> {
        repo.delete_assistant(id)
            .await
            .map_err(|e| format!("Failed to delete assistant: {}", e))
    }

    pub async fn list_assistants(
        repo: &dyn AssistantRepository,
    ) -> Result<Vec<AssistantModel>, String> {
        repo.list_assistants()
            .await
            .map_err(|e| format!("Failed to list assistants: {}", e))
    }

    pub async fn get_assistant(
        repo: &dyn AssistantRepository,
        id: &str,
    ) -> Result<Option<AssistantModel>, String> {
        repo.get_assistant(id)
            .await
            .map_err(|e| format!("Failed to get assistant: {}", e))
    }

    pub async fn search_assistants(
        repo: &dyn AssistantRepository,
        query: &str,
        limit: usize,
    ) -> Result<Vec<AssistantModel>, String> {
        let all = repo
            .list_assistants()
            .await
            .map_err(|e| format!("Failed to list assistants: {}", e))?;

        let lower_query = query.to_lowercase();

        let filtered: Vec<AssistantModel> = all
            .into_iter()
            .filter(|model| {
                let name_matches = model.name.to_lowercase().contains(&lower_query);

                let (desc_matches, prompt_matches) = if let Ok(config) = serde_json::from_str::<Value>(&model.config) {
                    let desc = config.get("description").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                    let prompt = config.get("systemPrompt").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();

                    (desc.contains(&lower_query), prompt.contains(&lower_query))
                } else {
                    (false, false)
                };

                name_matches || desc_matches || prompt_matches
            })
            .take(limit)
            .collect();

        Ok(filtered)
    }

    pub async fn batch_upsert_assistants(
        repo: &dyn AssistantRepository,
        assistants: Vec<AssistantUpsertPayload>,
    ) -> Result<Vec<AssistantModel>, String> {
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
