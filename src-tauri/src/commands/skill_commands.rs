use crate::services::skill_service::{self, SkillMetadata};
use crate::session::get_session_manager;
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
) -> Result<Vec<SkillMetadata>, String> {
    let global_dir_str = skill_service::get_configured_skills_directory().await?;
    let global_dir = PathBuf::from(global_dir_str);

    let mut assistant_dir = None;

    if let Some(id) = assistant_id {
        let session_manager = get_session_manager()?;
        assistant_dir = Some(
            session_manager
                .get_base_data_dir()
                .join("assistants")
                .join(&id)
                .join("skills"),
        );
    }

    skill_service::resolve_skills(global_dir, assistant_dir).await
}

#[tauri::command]
pub async fn scan_skills_directory(directory: String) -> Result<Vec<SkillMetadata>, String> {
    skill_service::scan_skills_directory(&PathBuf::from(directory)).await
}
