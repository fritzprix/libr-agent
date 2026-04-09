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
            .search_assistants(query)
            .await
            .map_err(|e| format!("Failed to search assistants: {}", e))?;

        let lower_query = query.to_lowercase();

        let filtered: Vec<AssistantModel> = all
            .into_iter()
            .filter(|model| {
                let name_matches = model.name.to_lowercase().contains(&lower_query);

                let (desc_matches, prompt_matches) =
                    if let Ok(config) = serde_json::from_str::<Value>(&model.config) {
                        let desc = config
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_lowercase();
                        let prompt = config
                            .get("systemPrompt")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_lowercase();

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::assistant::Model as AssistantModel;
    use crate::repositories::{AssistantRepository, DbError};
    use async_trait::async_trait;

    struct MockAssistantRepository {
        assistants: Vec<AssistantModel>,
    }

    #[async_trait]
    impl AssistantRepository for MockAssistantRepository {
        async fn create_assistant(&self, _id: String, _name: String, _config: String) -> Result<AssistantModel, DbError> { unimplemented!() }
        async fn get_assistant(&self, _id: &str) -> Result<Option<AssistantModel>, DbError> { unimplemented!() }
        async fn update_assistant(&self, _id: &str, _name: String, _config: String) -> Result<AssistantModel, DbError> { unimplemented!() }
        async fn delete_assistant(&self, _id: &str) -> Result<(), DbError> { unimplemented!() }
        async fn list_assistants(&self) -> Result<Vec<AssistantModel>, DbError> { unimplemented!() }
        async fn search_assistants(&self, _query: &str) -> Result<Vec<AssistantModel>, DbError> {
            Ok(self.assistants.clone())
        }
        async fn check_assistant_exists(&self, _name: &str) -> Result<bool, DbError> { unimplemented!() }
        async fn count_assistants(&self) -> Result<u64, DbError> { unimplemented!() }
    }

    #[tokio::test]
    async fn test_search_assistants_filtering() {
        let repo = MockAssistantRepository {
            assistants: vec![
                AssistantModel {
                    id: "1".into(),
                    name: "Rust Expert".into(),
                    config: r#"{"description":"Helps with Rust","systemPrompt":"You are a Rust dev"}"#.into(),
                    created_at: 0,
                    updated_at: 0,
                },
                AssistantModel {
                    id: "2".into(),
                    name: "Python Guru".into(),
                    config: r#"{"description":"Python scripts","systemPrompt":"You write python"}"#.into(),
                    created_at: 0,
                    updated_at: 0,
                },
                AssistantModel {
                    id: "3".into(),
                    name: "Generic Helper".into(),
                    config: r#"{"description":"No specific lang","systemPrompt":"Just a helper"}"#.into(),
                    created_at: 0,
                    updated_at: 0,
                },
            ],
        };

        // Match by name
        let res = AssistantService::search_assistants(&repo, "expert", 10).await.unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "1");

        // Match by description
        let res = AssistantService::search_assistants(&repo, "python scripts", 10).await.unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "2");

        // Match by systemPrompt
        let res = AssistantService::search_assistants(&repo, "just a helper", 10).await.unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "3");

        // Limit
        let res = AssistantService::search_assistants(&repo, "e", 1).await.unwrap();
        assert_eq!(res.len(), 1);
    }
}
