use crate::entity::playbook;
use crate::entity::session;
use crate::state::get_database_connection;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::command;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybookDto {
    pub id: String,
    pub assistant_id: String,
    pub session_id: String,
    pub goal: String,
    pub initial_command: Option<String>,
    pub workflow: Value,                 // JSON stored as TEXT
    pub success_criteria: Option<Value>, // JSON stored as TEXT
    pub created_at: i64,
    pub updated_at: i64,
    pub is_bookmarked: bool,
}

impl From<playbook::Model> for PlaybookDto {
    fn from(model: playbook::Model) -> Self {
        Self {
            id: model.id,
            assistant_id: model.assistant_id,
            session_id: model.session_id,
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

/// Helper to get assistant_id from session
async fn get_assistant_id_from_session(session_id: &str) -> Result<String, String> {
    let db = get_database_connection();

    let session_model = session::Entity::find_by_id(session_id)
        .one(db)
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

#[command]
pub async fn create_playbook(
    id: String,
    session_id: String,
    goal: String,
    initial_command: Option<String>,
    workflow: Value,
    success_criteria: Option<Value>,
) -> Result<PlaybookDto, String> {
    let db = get_database_connection();
    let now = chrono::Utc::now().timestamp_millis();

    // Get assistant_id from session
    let assistant_id = get_assistant_id_from_session(&session_id).await?;

    let playbook = playbook::ActiveModel {
        id: Set(id),
        assistant_id: Set(assistant_id),
        session_id: Set(session_id),
        goal: Set(goal),
        initial_command: Set(initial_command),
        workflow: Set(workflow.to_string()),
        success_criteria: Set(success_criteria.map(|s| s.to_string())),
        created_at: Set(now),
        updated_at: Set(now),
        is_bookmarked: Set(false),
    };

    let result = playbook
        .insert(db)
        .await
        .map_err(|e| format!("Failed to create playbook: {}", e))?;

    Ok(result.into())
}

#[command]
pub async fn update_playbook(
    id: String,
    session_id: String,
    goal: Option<String>,
    workflow: Option<Value>,
    success_criteria: Option<Value>,
) -> Result<PlaybookDto, String> {
    let db = get_database_connection();
    let now = chrono::Utc::now().timestamp_millis();

    // Get assistant_id from session
    let assistant_id = get_assistant_id_from_session(&session_id).await?;

    // Composite key (id, assistant_id)
    let mut playbook: playbook::ActiveModel = playbook::Entity::find_by_id((id, assistant_id))
        .one(db)
        .await
        .map_err(|e| format!("Failed to find playbook: {}", e))?
        .ok_or_else(|| "Playbook not found".to_string())?
        .into();

    if let Some(goal) = goal {
        playbook.goal = Set(goal);
    }
    if let Some(workflow) = workflow {
        playbook.workflow = Set(workflow.to_string());
    }
    if let Some(success_criteria) = success_criteria {
        playbook.success_criteria = Set(Some(success_criteria.to_string()));
    }
    playbook.updated_at = Set(now);

    let result = playbook
        .update(db)
        .await
        .map_err(|e| format!("Failed to update playbook: {}", e))?;

    Ok(result.into())
}

#[command]
pub async fn delete_playbook(id: String, session_id: String) -> Result<(), String> {
    let db = get_database_connection();

    // Get assistant_id from session
    let assistant_id = get_assistant_id_from_session(&session_id).await?;

    playbook::Entity::delete_by_id((id, assistant_id))
        .exec(db)
        .await
        .map_err(|e| format!("Failed to delete playbook: {}", e))?;
    Ok(())
}

#[command]
pub async fn toggle_playbook_bookmark(
    id: String,
    session_id: String,
    bookmarked: bool,
) -> Result<(), String> {
    let db = get_database_connection();

    // Get assistant_id from session
    let assistant_id = get_assistant_id_from_session(&session_id).await?;

    // Composite key (id, assistant_id)
    let mut playbook: playbook::ActiveModel = playbook::Entity::find_by_id((id, assistant_id))
        .one(db)
        .await
        .map_err(|e| format!("Failed to find playbook: {}", e))?
        .ok_or_else(|| "Playbook not found".to_string())?
        .into();

    playbook.is_bookmarked = Set(bookmarked);
    playbook.updated_at = Set(chrono::Utc::now().timestamp_millis());

    playbook
        .update(db)
        .await
        .map_err(|e| format!("Failed to toggle bookmark: {}", e))?;

    Ok(())
}

#[command]
pub async fn list_playbooks(
    session_id: Option<String>,
    sort_by: Option<String>,
    bookmark_first: Option<bool>,
) -> Result<Vec<PlaybookDto>, String> {
    let db = get_database_connection();

    let query = playbook::Entity::find();

    let query = if let Some(sid) = session_id {
        // Get assistant_id from session and filter by it
        let assistant_id = get_assistant_id_from_session(&sid).await?;
        query.filter(playbook::Column::AssistantId.eq(assistant_id))
    } else {
        query
    };

    let query = if bookmark_first.unwrap_or(false) {
        query.order_by_desc(playbook::Column::IsBookmarked)
    } else {
        query
    };

    let query = match sort_by.as_deref() {
        Some("assistant") => query.order_by_asc(playbook::Column::AssistantId),
        _ => query.order_by_desc(playbook::Column::CreatedAt),
    };

    let playbooks = query
        .all(db)
        .await
        .map_err(|e| format!("Failed to list playbooks: {}", e))?;

    Ok(playbooks.into_iter().map(|p| p.into()).collect())
}
