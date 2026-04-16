use crate::services::skill_service::{self, SkillImportPreview, SkillImportResult};

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

#[tauri::command]
pub async fn preview_user_skill_import(file_path: String) -> Result<SkillImportPreview, String> {
    skill_service::preview_user_skill_import(file_path).await
}

#[tauri::command]
pub async fn import_user_skills(
    file_path: String,
    overwrite_existing: bool,
) -> Result<SkillImportResult, String> {
    skill_service::import_user_skills(file_path, overwrite_existing).await
}

#[tauri::command]
pub async fn preview_github_skill_install(repo_url: String) -> Result<SkillImportPreview, String> {
    skill_service::preview_github_skill_install(repo_url).await
}

#[tauri::command]
pub async fn install_github_skills(
    repo_url: String,
    overwrite_existing: bool,
) -> Result<SkillImportResult, String> {
    skill_service::install_github_skills(repo_url, overwrite_existing).await
}

#[tauri::command]
pub async fn delete_user_skill(skill_name: String) -> Result<String, String> {
    skill_service::delete_user_skill(skill_name).await
}

#[tauri::command]
pub async fn reset_user_skills() -> Result<String, String> {
    skill_service::reset_user_skills().await
}
