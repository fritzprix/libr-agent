use crate::entity::playbook::Model as PlaybookModel;
use crate::mcp::builtin::playbook::{
    parse_workflow_payload, serialize_default_target_session_column, serialize_workflow_steps,
    TargetSessionConfig,
};
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
    pub workflow: Value, // steps array JSON
    pub success_criteria: Option<Value>,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_bookmarked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_target_session: Option<Value>,
}

impl From<PlaybookModel> for PlaybookDto {
    fn from(model: PlaybookModel) -> Self {
        let (steps, envelope_target) = parse_workflow_payload(&model.workflow);
        let workflow = serde_json::to_value(&steps).unwrap_or(Value::Array(vec![]));

        let default_target_session = model
            .default_target_session
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
            .or_else(|| envelope_target.and_then(|t| serde_json::to_value(t).ok()));

        Self {
            id: model.id,
            assistant_id: model.assistant_id,
            goal: model.goal,
            initial_command: model.initial_command,
            workflow,
            success_criteria: model
                .success_criteria
                .and_then(|s| serde_json::from_str(&s).ok()),
            created_at: model.created_at,
            updated_at: model.updated_at,
            is_bookmarked: model.is_bookmarked,
            default_target_session,
        }
    }
}

/// Split FE/API workflow (steps array or legacy envelope) + optional pin into storage columns.
fn normalize_for_storage(
    workflow: Value,
    default_target_session: Option<Value>,
) -> Result<(String, Option<String>), String> {
    let workflow_str = workflow.to_string();
    let (steps, envelope_target) = parse_workflow_payload(&workflow_str);

    let pin_cfg: Option<TargetSessionConfig> = match default_target_session {
        Some(Value::Null) => None,
        Some(v) => Some(
            serde_json::from_value(v)
                .map_err(|e| format!("Invalid defaultTargetSession: {e}"))?,
        ),
        None => envelope_target,
    };

    let steps_json = serialize_workflow_steps(&steps)
        .map_err(|e| format!("Failed to serialize workflow steps: {e}"))?;
    let pin_json = serialize_default_target_session_column(pin_cfg.as_ref())
        .map_err(|e| format!("Failed to serialize defaultTargetSession: {e}"))?;

    Ok((steps_json, pin_json))
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
        default_target_session: Option<Value>,
    ) -> Result<PlaybookModel, String> {
        let assistant_id = Self::get_assistant_id_from_session(session_repo, session_id).await?;
        let (steps_json, pin_json) = normalize_for_storage(workflow, default_target_session)?;

        repo.create_playbook(id, assistant_id, goal, steps_json, pin_json)
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
        default_target_session: Option<Value>,
    ) -> Result<PlaybookModel, String> {
        let assistant_id = Self::get_assistant_id_from_session(session_repo, session_id).await?;

        let (workflow_json, pin_update) = match (workflow, default_target_session) {
            (None, None) => (None, None),
            (Some(wf), pin) => {
                let (steps, pin_json) = normalize_for_storage(wf, pin)?;
                (Some(steps), Some(pin_json))
            }
            (None, Some(pin)) => {
                let pin_cfg: Option<TargetSessionConfig> = if pin.is_null() {
                    None
                } else {
                    Some(
                        serde_json::from_value(pin)
                            .map_err(|e| format!("Invalid defaultTargetSession: {e}"))?,
                    )
                };
                let pin_json = serialize_default_target_session_column(pin_cfg.as_ref())
                    .map_err(|e| format!("Failed to serialize defaultTargetSession: {e}"))?;
                (None, Some(pin_json))
            }
        };

        repo.update_playbook(
            id,
            &assistant_id,
            goal,
            workflow_json,
            pin_update,
            None,
        )
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
        repo.update_playbook(id, assistant_id, None, None, None, Some(bookmarked))
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
