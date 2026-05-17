use crate::services::skill_service::{self, ManagedSkillsOverview, SkillMetadata};
use std::path::{Path, PathBuf};

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

#[tauri::command]
pub async fn get_aggregated_skills(
    assistant_id: Option<String>,
    session_id: Option<String>,
    workspace_path: Option<String>,
) -> Result<Vec<SkillMetadata>, String> {
    let (system_dir, user_dir, assistant_dir, workspace_dir) =
        skill_service::resolve_skill_directories(
            assistant_id.as_deref(),
            session_id.as_deref(),
            workspace_path.as_deref().map(Path::new),
        )
        .await?;

    skill_service::resolve_skills(system_dir, user_dir, assistant_dir, workspace_dir).await
}

#[tauri::command]
pub async fn get_managed_skills_overview() -> Result<ManagedSkillsOverview, String> {
    skill_service::get_managed_skills_overview().await
}

#[tauri::command]
pub async fn scan_skills_directory(directory: String) -> Result<Vec<SkillMetadata>, String> {
    skill_service::scan_skills_directory(&PathBuf::from(directory)).await
}

/// Returns the full content of a skill's SKILL.md file.
/// `skill_path` is the absolute path as returned in `SkillMetadata.path`.
#[tauri::command]
pub async fn get_skill_content(
    skill_path: String,
    assistant_id: Option<String>,
    session_id: Option<String>,
    workspace_path: Option<String>,
) -> Result<String, String> {
    skill_service::get_skill_content(skill_path, assistant_id, session_id, workspace_path).await
}
