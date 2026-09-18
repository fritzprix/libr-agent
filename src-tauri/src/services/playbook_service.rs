use crate::entity::playbook::Model as PlaybookModel;
use crate::repositories::{PaginationParams, PlaybookRepository, SessionRepository};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookDto {
    pub id: String,
    pub assistant_id: String,
    pub goal: String,
    pub initial_command: Option<String>,
    pub workflow: Value,                 // JSON stored as TEXT
    pub success_criteria: Option<Value>, // JSON stored as TEXT
    pub created_at: i64,
    pub updated_at: i64,
    pub is_bookmarked: bool,
}

impl From<PlaybookModel> for PlaybookDto {
    fn from(model: PlaybookModel) -> Self {
        Self {
            id: model.id,
            assistant_id: model.assistant_id,
            goal: model.goal,
            initial_command: model.initial_command,
            workflow: serde_json::from_str(&model.workflow).unwrap_or(Value::Null),
            success_criteria: model
                .success_criteria
                .and_then(|s| serde_json::from_str(&s).ok()),
            created_at: model.created_at,
            updated_at: model.updated_at,
            is_bookmarked: model.is_bookmarked,
        }
    }
}

pub struct PlaybookService;

impl PlaybookService {
    pub async fn create_playbook(
        repo: &dyn PlaybookRepository,
        session_repo: &dyn SessionRepository,
        id: String,
        session_id: &str,
        goal: String,
        workflow: Value,
    ) -> Result<PlaybookModel, String> {
        let assistant_id = Self::get_assistant_id_from_session(session_repo, session_id).await?;

        repo.create_playbook(id, assistant_id, goal, workflow.to_string())
            .await
            .map_err(|e| format!("Failed to create playbook: {}", e))
    }

    pub async fn update_playbook(
        repo: &dyn PlaybookRepository,
        session_repo: &dyn SessionRepository,
        id: &str,
        session_id: &str,
        goal: Option<String>,
        workflow: Option<Value>,
    ) -> Result<PlaybookModel, String> {
        let assistant_id = Self::get_assistant_id_from_session(session_repo, session_id).await?;

        let workflow_str = match workflow {
            Some(Value::Array(steps)) => {
                // If workflow is provided as a plain array of steps, preserve existing session targeting and step configs
                if let Ok(Some(existing_model)) = repo.get_playbook(id, &assistant_id).await {
                    let existing_pb =
                        crate::mcp::builtin::playbook::Playbook::from_model(&existing_model);

                    let merged_steps: Vec<Value> = steps
                        .into_iter()
                        .map(|mut step_val| {
                            if let Some(step_obj) = step_val.as_object_mut() {
                                if let Some(step_id) =
                                    step_obj.get("stepId").and_then(|id| id.as_str())
                                {
                                    if let Some(existing_step) = existing_pb
                                        .workflow
                                        .iter()
                                        .find(|s| s.step_id.as_deref() == Some(step_id))
                                    {
                                        if !step_obj.contains_key("promptTemplate") {
                                            if let Some(ref pt) = existing_step.prompt_template {
                                                step_obj.insert(
                                                    "promptTemplate".to_string(),
                                                    Value::String(pt.clone()),
                                                );
                                            }
                                        }
                                        if !step_obj.contains_key("sessionSlot") {
                                            if let Some(ref ss) = existing_step.session_slot {
                                                step_obj.insert(
                                                    "sessionSlot".to_string(),
                                                    Value::String(ss.clone()),
                                                );
                                            }
                                        }
                                        if !step_obj.contains_key("targetSession") {
                                            if let Some(ref ts) = existing_step.target_session {
                                                if let Ok(ts_val) = serde_json::to_value(ts) {
                                                    step_obj.insert(
                                                        "targetSession".to_string(),
                                                        ts_val,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            step_val
                        })
                        .collect();

                    if existing_pb.default_target_session.is_some()
                        || existing_pb.session_slots.is_some()
                    {
                        Some(
                            serde_json::json!({
                                "steps": merged_steps,
                                "defaultTargetSession": existing_pb.default_target_session,
                                "sessionSlots": existing_pb.session_slots,
                            })
                            .to_string(),
                        )
                    } else {
                        Some(Value::Array(merged_steps).to_string())
                    }
                } else {
                    Some(Value::Array(steps).to_string())
                }
            }
            Some(v) => Some(v.to_string()),
            None => None,
        };

        repo.update_playbook(id, &assistant_id, goal, workflow_str, None)
            .await
            .map_err(|e| format!("Failed to update playbook: {}", e))
    }

    pub async fn delete_playbook(
        repo: &dyn PlaybookRepository,
        id: &str,
        assistant_id: &str,
    ) -> Result<(), String> {
        repo.delete_playbook(id, assistant_id)
            .await
            .map_err(|e| format!("Failed to delete playbook: {}", e))
    }

    pub async fn get_playbook(
        repo: &dyn PlaybookRepository,
        id: &str,
        assistant_id: &str,
    ) -> Result<Option<PlaybookModel>, String> {
        repo.get_playbook(id, assistant_id)
            .await
            .map_err(|e| format!("Failed to get playbook: {}", e))
    }

    pub async fn toggle_playbook_bookmark(
        repo: &dyn PlaybookRepository,
        id: &str,
        assistant_id: &str,
        bookmarked: bool,
    ) -> Result<(), String> {
        repo.update_playbook(id, assistant_id, None, None, Some(bookmarked))
            .await
            .map_err(|e| format!("Failed to toggle bookmark: {}", e))?;
        Ok(())
    }

    /// Helper to get assistant_id from session
    pub async fn get_assistant_id_from_session(
        session_repo: &dyn SessionRepository,
        session_id: &str,
    ) -> Result<String, String> {
        let session = session_repo
            .get_session(session_id)
            .await
            .map_err(|e| format!("Failed to query session: {}", e))?
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        crate::agent::extract_assistant_id_from_session(&session)
            .ok_or_else(|| format!("Session {} has no assistant configuration", session_id))
    }

    pub async fn list_playbooks(
        repo: &dyn PlaybookRepository,
        assistant_id: String,
        sort_by: Option<String>,
        bookmark_first: Option<bool>,
    ) -> Result<Vec<PlaybookDto>, String> {
        let assistant_id_filter = if assistant_id.is_empty() {
            None
        } else {
            Some(assistant_id.as_str())
        };

        // For now, use list_playbooks without pagination
        // Full pagination support can be added if needed
        let pagination = PaginationParams {
            page: 1,
            page_size: 1000,
        };

        let page = repo
            .list_playbooks(assistant_id_filter, pagination)
            .await
            .map_err(|e| format!("Failed to list playbooks: {}", e))?;

        let mut playbooks: Vec<PlaybookDto> = page.items.into_iter().map(|p| p.into()).collect();

        // Apply sorting
        let bookmark_first = bookmark_first.unwrap_or(false);
        match sort_by.as_deref() {
            Some("assistant") => playbooks.sort_by(|a, b| {
                if bookmark_first && a.is_bookmarked != b.is_bookmarked {
                    b.is_bookmarked.cmp(&a.is_bookmarked)
                } else {
                    a.assistant_id.cmp(&b.assistant_id)
                }
            }),
            _ => playbooks.sort_by(|a, b| {
                if bookmark_first && a.is_bookmarked != b.is_bookmarked {
                    b.is_bookmarked.cmp(&a.is_bookmarked)
                } else {
                    b.created_at.cmp(&a.created_at)
                }
            }),
        };

        Ok(playbooks)
    }
}
