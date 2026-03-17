use crate::services::playbook_service::{PlaybookDto, PlaybookService};
use crate::state::{get_playbook_repository, get_session_repository};
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
    let result = PlaybookService::create_playbook(
        get_playbook_repository(),
        get_session_repository(),
        id,
        &session_id,
        goal,
        workflow,
    )
    .await?;
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
    let result = PlaybookService::update_playbook(
        get_playbook_repository(),
        get_session_repository(),
        &id,
        &session_id,
        goal,
        workflow,
    )
    .await?;
    Ok(result.into())
}

#[command]
pub async fn delete_playbook(id: String, assistant_id: String) -> Result<(), String> {
    PlaybookService::delete_playbook(get_playbook_repository(), &id, &assistant_id).await
}

#[command]
pub async fn get_playbook(id: String, assistant_id: String) -> Result<Option<PlaybookDto>, String> {
    let result =
        PlaybookService::get_playbook(get_playbook_repository(), &id, &assistant_id).await?;
    Ok(result.map(|m| m.into()))
}

#[command]
pub async fn toggle_playbook_bookmark(
    id: String,
    assistant_id: String,
    bookmarked: bool,
) -> Result<(), String> {
    PlaybookService::toggle_playbook_bookmark(
        get_playbook_repository(),
        &id,
        &assistant_id,
        bookmarked,
    )
    .await
}

#[command]
pub async fn list_playbooks(
    assistant_id: String,
    sort_by: Option<String>,
    bookmark_first: Option<bool>,
) -> Result<Vec<PlaybookDto>, String> {
    PlaybookService::list_playbooks(
        get_playbook_repository(),
        assistant_id,
        sort_by,
        bookmark_first,
    )
    .await
}
