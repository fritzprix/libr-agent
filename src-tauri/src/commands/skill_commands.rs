use crate::services::skill_service::{self, SkillMetadata};
use std::path::PathBuf;

#[tauri::command]
pub async fn get_default_skills_directory() -> Result<String, String> {
    skill_service::get_default_skills_directory().await
}

#[tauri::command]
pub async fn open_skills_directory_in_explorer(directory: Option<String>) -> Result<(), String> {
    let skills_dir = if let Some(dir) = directory {
        PathBuf::from(dir)
    } else {
        let path_str = skill_service::get_default_skills_directory().await?;
        PathBuf::from(path_str)
    };

    if !skills_dir.exists() {
        return Err("Skills directory does not exist".to_string());
    }

    crate::utils::fs::open_in_file_manager(&skills_dir)
}

pub async fn get_configured_skills_directory() -> Result<String, String> {
    skill_service::get_configured_skills_directory().await
}

#[tauri::command]
pub async fn get_aggregated_skills(
    assistant_id: Option<String>,
    session_id: Option<String>,
    workspace_path: Option<String>,
) -> Result<Vec<SkillMetadata>, String> {
    let global_dir_str = skill_service::get_configured_skills_directory().await?;
    let global_dir = PathBuf::from(global_dir_str);

    let assistant_dir = assistant_id
        .as_deref()
        .map(skill_service::get_assistant_skills_directory)
        .transpose()?;

    let workspace_dir = if let Some(path) = workspace_path {
        Some(skill_service::get_workspace_skills_directory_from_path(
            &PathBuf::from(path),
        ))
    } else if let Some(id) = session_id {
        Some(skill_service::get_workspace_skills_directory_for_session(
            &id,
        )?)
    } else {
        None
    };

    skill_service::resolve_skills(global_dir, assistant_dir, workspace_dir).await
}

#[tauri::command]
pub async fn scan_skills_directory(directory: String) -> Result<Vec<SkillMetadata>, String> {
    skill_service::scan_skills_directory(&PathBuf::from(directory)).await
}

/// Returns the full content of a skill's SKILL.md file.
/// `skill_path` is the absolute path as returned in `SkillMetadata.path`.
#[tauri::command]
pub async fn get_skill_content(skill_path: String) -> Result<String, String> {
    skill_service::get_skill_content(skill_path).await
}

#[tauri::command]
pub async fn list_workspace_file_paths(
    session_id: String,
    max_depth: usize,
) -> Result<Vec<String>, String> {
    crate::agent::references::list_workspace_relative_paths(&session_id, max_depth).await
}
