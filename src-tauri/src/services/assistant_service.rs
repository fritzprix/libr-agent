use crate::entity::assistant::Model as AssistantModel;
use crate::repositories::AssistantRepository;
use serde_json::Value;

pub struct AssistantUpsertPayload {
    pub id: String,
    pub name: String,
    pub config: Value,
}

pub struct AssistantService;

pub fn normalize_assistant_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Assistant name cannot be blank".to_string());
    }
    Ok(trimmed.to_string())
}

pub fn normalize_optional_assistant_name(name: Option<String>) -> Result<Option<String>, String> {
    name.map(|value| normalize_assistant_name(&value))
        .transpose()
}

impl AssistantService {
    pub async fn create_assistant(
        repo: &dyn AssistantRepository,
        id: String,
        name: String,
        config: Value,
    ) -> Result<AssistantModel, String> {
        // #2923 D2: `plugin:` ids and the pluginSource marker are reserved
        // for plugin install flows; user creation is rejected.
        crate::services::skill_service::reject_plugin_agent_mutation(&id, Some(&config))?;
        let name = normalize_assistant_name(&name)?;
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
        // #2923 D2: plugin-owned assistants are read-only for users.
        crate::services::skill_service::reject_plugin_agent_mutation(id, config.as_ref())?;
        let name = normalize_optional_assistant_name(name)?;
        let config_str = config.map(|c| c.to_string());

        repo.update_assistant(id, name, config_str)
            .await
            .map_err(|e| format!("Failed to update assistant: {}", e))
    }

    pub async fn delete_assistant(repo: &dyn AssistantRepository, id: &str) -> Result<(), String> {
        // #2923 D2: removed only together with the owning plugin.
        crate::services::skill_service::reject_plugin_agent_mutation(id, None)?;
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
            // #2923 D2: an import/batch upsert may not create or overwrite
            // plugin-owned assistants, and may not spoof the marker.
            crate::services::skill_service::reject_plugin_agent_mutation(
                &payload.id,
                Some(&payload.config),
            )?;
            let normalized_name = normalize_assistant_name(&payload.name)?;
            let config_str = payload.config.to_string();

            let update_result = repo
                .update_assistant(
                    &payload.id,
                    Some(normalized_name.clone()),
                    Some(config_str.clone()),
                )
                .await;

            let result = match update_result {
                Ok(model) => model,
                Err(crate::repositories::DbError::NotFound(_)) => repo
                    .create_assistant(payload.id.clone(), normalized_name, config_str)
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
    use crate::entity::assistant::Model as AssistantModel;
    use crate::repositories::{AssistantRepository, DbError};
    use async_trait::async_trait;

    struct MockAssistantRepository {
        assistants: Vec<AssistantModel>,
    }

    #[async_trait]
    impl AssistantRepository for MockAssistantRepository {
        async fn create_assistant(
            &self,
            _id: String,
            _name: String,
            _config: String,
        ) -> Result<AssistantModel, DbError> {
            unimplemented!()
        }
        async fn get_assistant(&self, _id: &str) -> Result<Option<AssistantModel>, DbError> {
            unimplemented!()
        }
        async fn update_assistant(
            &self,
            _id: &str,
            _name: Option<String>,
            _config: Option<String>,
        ) -> Result<AssistantModel, DbError> {
            unimplemented!()
        }
        async fn delete_assistant(&self, _id: &str) -> Result<(), DbError> {
            unimplemented!()
        }
        async fn list_assistants(&self) -> Result<Vec<AssistantModel>, DbError> {
            unimplemented!()
        }
        async fn list_assistants_paginated(
            &self,
            limit: u64,
            offset: u64,
        ) -> Result<Vec<AssistantModel>, DbError> {
            Ok(self
                .assistants
                .iter()
                .skip(offset as usize)
                .take(limit as usize)
                .cloned()
                .collect())
        }
        async fn search_assistants(&self, _query: &str) -> Result<Vec<AssistantModel>, DbError> {
            Ok(self.assistants.clone())
        }
        async fn check_assistant_exists(&self, _name: &str) -> Result<bool, DbError> {
            unimplemented!()
        }
        async fn count_assistants(&self) -> Result<u64, DbError> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_search_assistants_filtering() {
        let repo = MockAssistantRepository {
            assistants: vec![
                AssistantModel {
                    id: "1".into(),
                    name: "Rust Expert".into(),
                    config:
                        r#"{"description":"Helps with Rust","systemPrompt":"You are a Rust dev"}"#
                            .into(),
                    created_at: 0,
                    updated_at: 0,
                },
                AssistantModel {
                    id: "2".into(),
                    name: "Python Guru".into(),
                    config: r#"{"description":"Python scripts","systemPrompt":"You write python"}"#
                        .into(),
                    created_at: 0,
                    updated_at: 0,
                },
                AssistantModel {
                    id: "3".into(),
                    name: "Generic Helper".into(),
                    config: r#"{"description":"No specific lang","systemPrompt":"Just a helper"}"#
                        .into(),
                    created_at: 0,
                    updated_at: 0,
                },
            ],
        };

        // Match by name
        let res = AssistantService::search_assistants(&repo, "expert", 10)
            .await
            .unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "1");

        // Match by description
        let res = AssistantService::search_assistants(&repo, "python scripts", 10)
            .await
            .unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "2");

        // Match by systemPrompt
        let res = AssistantService::search_assistants(&repo, "just a helper", 10)
            .await
            .unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "3");

        // Limit
        let res = AssistantService::search_assistants(&repo, "e", 1)
            .await
            .unwrap();
        assert_eq!(res.len(), 1);
    }
}

// #2923 (D2, gatekeeper G1.3): every AssistantService mutation path runs
// the plugin guard BEFORE touching the store. The recording repository
// proves the store is never reached for plugin-owned ids.
#[cfg(test)]
mod plugin_guard_tests {
    use super::*;
    use crate::repositories::DbError;
    use async_trait::async_trait;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingRepository {
        calls: Mutex<Vec<String>>,
    }

    impl RecordingRepository {
        fn record(&self, call: &str) {
            self.calls.lock().unwrap().push(call.to_string());
        }
        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    fn dummy_model(id: &str) -> AssistantModel {
        AssistantModel {
            id: id.to_string(),
            name: "stored".to_string(),
            config: "{}".to_string(),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[async_trait]
    impl AssistantRepository for RecordingRepository {
        async fn create_assistant(
            &self,
            id: String,
            _name: String,
            _config: String,
        ) -> Result<AssistantModel, DbError> {
            self.record(&format!("create:{id}"));
            Ok(dummy_model(&id))
        }
        async fn get_assistant(&self, id: &str) -> Result<Option<AssistantModel>, DbError> {
            self.record(&format!("get:{id}"));
            Ok(Some(dummy_model(id)))
        }
        async fn update_assistant(
            &self,
            id: &str,
            _name: Option<String>,
            _config: Option<String>,
        ) -> Result<AssistantModel, DbError> {
            self.record(&format!("update:{id}"));
            Ok(dummy_model(id))
        }
        async fn delete_assistant(&self, id: &str) -> Result<(), DbError> {
            self.record(&format!("delete:{id}"));
            Ok(())
        }
        async fn list_assistants(&self) -> Result<Vec<AssistantModel>, DbError> {
            self.record("list");
            Ok(Vec::new())
        }
        async fn list_assistants_paginated(
            &self,
            _limit: u64,
            _offset: u64,
        ) -> Result<Vec<AssistantModel>, DbError> {
            Ok(Vec::new())
        }
        async fn search_assistants(&self, _query: &str) -> Result<Vec<AssistantModel>, DbError> {
            Ok(Vec::new())
        }
        async fn check_assistant_exists(&self, _name: &str) -> Result<bool, DbError> {
            Ok(false)
        }
        async fn count_assistants(&self) -> Result<u64, DbError> {
            Ok(0)
        }
    }

    fn plugin_id() -> String {
        crate::services::skill_service::plugin_agent_id("plugin-aa", "reviewer")
    }

    #[tokio::test]
    async fn all_mutations_reject_plugin_owned_ids_before_touching_the_store() {
        let repo = RecordingRepository::default();
        let owned = plugin_id();
        let empty = serde_json::json!({});

        assert!(AssistantService::create_assistant(
            &repo,
            owned.clone(),
            "n".into(),
            empty.clone()
        )
        .await
        .is_err());
        assert!(
            AssistantService::update_assistant(&repo, &owned, None, None)
                .await
                .is_err()
        );
        assert!(AssistantService::delete_assistant(&repo, &owned)
            .await
            .is_err());
        assert!(AssistantService::batch_upsert_assistants(
            &repo,
            vec![AssistantUpsertPayload {
                id: owned.clone(),
                name: "n".into(),
                config: empty.clone(),
            }]
        )
        .await
        .is_err());
        assert!(
            repo.calls().is_empty(),
            "store must never be reached: {:?}",
            repo.calls()
        );

        // Spoofing the ownership marker on a user id is rejected the same way.
        let spoofed = serde_json::json!({ "pluginSource": "plugin:plugin-aa" });
        assert!(
            AssistantService::update_assistant(&repo, "user-row", None, Some(spoofed.clone()))
                .await
                .is_err()
        );
        assert!(
            AssistantService::create_assistant(&repo, "user-row".into(), "n".into(), spoofed)
                .await
                .is_err()
        );
        assert!(AssistantService::batch_upsert_assistants(
            &repo,
            vec![AssistantUpsertPayload {
                id: "user-row".into(),
                name: "n".into(),
                config: serde_json::json!({ "pluginSource": "plugin:plugin-aa" }),
            }]
        )
        .await
        .is_err());
        assert!(
            repo.calls().is_empty(),
            "store must never be reached: {:?}",
            repo.calls()
        );
    }

    #[tokio::test]
    async fn normal_mutations_still_reach_the_store() {
        let repo = RecordingRepository::default();
        AssistantService::delete_assistant(&repo, "user-row")
            .await
            .unwrap();
        assert_eq!(repo.calls(), vec!["delete:user-row".to_string()]);
    }
}
