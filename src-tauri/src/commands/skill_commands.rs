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

    // UI callers (e.g. @skill: autocomplete) must see skill folders added on disk
    // since the last scan. Runtime paths that call resolve_skills keep the cache.
    skill_service::clear_skill_scan_cache_entries();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn get_aggregated_skills_sees_skill_folder_added_after_first_call() {
        let workspace = tempfile::tempdir().unwrap();
        let workspace_path = workspace.path().to_string_lossy().to_string();

        let before = get_aggregated_skills(None, None, Some(workspace_path.clone()))
            .await
            .unwrap();
        assert!(!before.iter().any(|skill| skill.name == "late-added-skill"));

        let skill_dir = skill_service::get_workspace_skills_directory_from_path(workspace.path())
            .join("late-added-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: late-added-skill\ndescription: added on disk\n---\n# Late\n",
        )
        .unwrap();

        let after = get_aggregated_skills(None, None, Some(workspace_path))
            .await
            .unwrap();
        assert!(after.iter().any(|skill| skill.name == "late-added-skill"));
    }
}
