use crate::services::skill_service;

#[tauri::command]
pub async fn copy_global_to_assistant(
    assistant_id: String,
    skill_name: String,
) -> Result<String, String> {
    skill_service::copy_global_to_assistant(assistant_id, skill_name).await
}

#[tauri::command]
pub async fn delete_assistant_skill(
    assistant_id: String,
    skill_name: String,
) -> Result<String, String> {
    skill_service::delete_assistant_skill(assistant_id, skill_name).await
}

#[tauri::command]
pub async fn reset_assistant_skills(assistant_id: String) -> Result<String, String> {
    skill_service::reset_assistant_skills(assistant_id).await
}

#[tauri::command]
pub async fn import_assistant_skills(
    assistant_id: String,
    file_path: String,
) -> Result<String, String> {
    skill_service::import_assistant_skills(assistant_id, file_path).await
}
