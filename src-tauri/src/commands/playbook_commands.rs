use crate::repositories::PlaybookRepository;
use crate::services::playbook_service::{PlaybookDto, PlaybookService};
use crate::state::get_playbook_repository;
use serde_json::Value;
use tauri::command;

#[command]
pub async fn create_playbook(
    id: String,
    session_id: String,
    goal: String,
    _initial_command: Option<String>,
    workflow: Value,
    _success_criteria: Option<Value>,
) -> Result<PlaybookDto, String> {
    let repo = get_playbook_repository();

    // Get assistant_id from session
    let assistant_id = PlaybookService::get_assistant_id_from_session(&session_id).await?;

    let result = repo
        .create_playbook(id, assistant_id, goal, workflow.to_string())
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
    _success_criteria: Option<Value>,
) -> Result<PlaybookDto, String> {
    let repo = get_playbook_repository();

    // Get assistant_id from session
    let assistant_id = PlaybookService::get_assistant_id_from_session(&session_id).await?;

    let result = repo
        .update_playbook(
            &id,
            &assistant_id,
            goal,
            workflow.map(|v| v.to_string()),
            None,
        )
        .await
        .map_err(|e| format!("Failed to update playbook: {}", e))?;

    Ok(result.into())
}

#[command]
pub async fn delete_playbook(id: String, assistant_id: String) -> Result<(), String> {
    let repo = get_playbook_repository();

    repo.delete_playbook(&id, &assistant_id)
        .await
        .map_err(|e| format!("Failed to delete playbook: {}", e))?;
    Ok(())
}

#[command]
pub async fn get_playbook(id: String, assistant_id: String) -> Result<Option<PlaybookDto>, String> {
    let repo = get_playbook_repository();

    let result = repo
        .get_playbook(&id, &assistant_id)
        .await
        .map_err(|e| format!("Failed to get playbook: {}", e))?;

    Ok(result.map(|m| m.into()))
}

#[command]
pub async fn toggle_playbook_bookmark(
    id: String,
    assistant_id: String,
    bookmarked: bool,
) -> Result<(), String> {
    let repo = get_playbook_repository();

    repo.update_playbook(&id, &assistant_id, None, None, Some(bookmarked))
        .await
        .map_err(|e| format!("Failed to toggle bookmark: {}", e))?;

    Ok(())
}

#[command]
pub async fn list_playbooks(
    assistant_id: String,
    sort_by: Option<String>,
    bookmark_first: Option<bool>,
) -> Result<Vec<PlaybookDto>, String> {
    PlaybookService::list_playbooks(assistant_id, sort_by, bookmark_first).await
}
