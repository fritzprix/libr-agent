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
    /// Helper to get assistant_id from session
    pub async fn get_assistant_id_from_session(session_id: &str) -> Result<String, String> {
        let session_model = crate::state::get_session_repository()
            .get_session(session_id)
            .await
            .map_err(|e| format!("Failed to query session: {}", e))?
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let agent_config_str = session_model
            .agent_config
            .ok_or_else(|| format!("Session {} has no agent config", session_id))?;

        let agent_config: Value = serde_json::from_str(&agent_config_str)
            .map_err(|e| format!("Failed to parse agent config: {}", e))?;

        // Try "assistantId" first (test data), fallback to "id" (production AgentConfig serialization)
        let assistant_id = agent_config
            .get("assistantId")
            .or_else(|| agent_config.get("id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                format!(
                    "No id/assistantId in agent config for session {}",
                    session_id
                )
            })?;

        Ok(assistant_id.to_string())
    }

    pub async fn list_playbooks(
        agent_id: String,
        sort_by: Option<String>,
        bookmark_first: Option<bool>,
    ) -> Result<Vec<PlaybookDto>, String> {
        let repo = crate::state::get_playbook_repository();

        let assistant_id = if agent_id.is_empty() {
            None
        } else {
            Some(agent_id.as_str())
        };

        // For now, use list_playbooks without pagination
        // Full pagination support can be added if needed
        let pagination = PaginationParams {
            page: 1,
            page_size: 1000,
        };

        let page = repo
            .list_playbooks(assistant_id, pagination)
            .await
            .map_err(|e| format!("Failed to list playbooks: {}", e))?;

        let mut playbooks: Vec<PlaybookDto> = page.items.into_iter().map(|p| p.into()).collect();

        // Apply sorting
        if bookmark_first.unwrap_or(false) {
            playbooks.sort_by(|a, b| b.is_bookmarked.cmp(&a.is_bookmarked));
        }

        match sort_by.as_deref() {
            Some("assistant") => playbooks.sort_by(|a, b| a.assistant_id.cmp(&b.assistant_id)),
            _ => playbooks.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
        };

        Ok(playbooks)
    }
}