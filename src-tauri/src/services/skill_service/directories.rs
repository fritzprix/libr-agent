use super::contracts::{
    ManagedSkillsOverview, SkillMetadata, LEGACY_SYSTEM_SKILLS_DIR_NAME, SYSTEM_SKILLS_DIR_NAME,
    USER_SKILLS_DIR_NAME,
};
use super::scan_skills_internal;
use crate::session::get_session_manager;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub async fn get_default_skills_directory() -> Result<String, String> {
    Ok(get_user_skills_directory()?.to_string_lossy().to_string())
}

pub async fn get_configured_skills_directory() -> Result<String, String> {
    get_default_skills_directory().await
}

pub fn get_system_skills_directory() -> Result<PathBuf, String> {
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join(SYSTEM_SKILLS_DIR_NAME))
}

pub fn get_user_skills_directory() -> Result<PathBuf, String> {
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join(USER_SKILLS_DIR_NAME))
}

pub fn get_legacy_global_skills_directory() -> Result<PathBuf, String> {
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join(LEGACY_SYSTEM_SKILLS_DIR_NAME))
}

fn merge_skill_layers(skill_layers: Vec<Vec<SkillMetadata>>) -> Vec<SkillMetadata> {
    let mut merged_skills = Vec::new();
    let mut seen_names = HashSet::new();

    for skill_layer in skill_layers {
        for skill in skill_layer {
            let normalized = skill.name.to_lowercase();
            if seen_names.insert(normalized) {
                merged_skills.push(skill);
            }
        }
    }

    merged_skills.sort_by_cached_key(|skill| skill.name.to_lowercase());
    merged_skills
}

pub async fn get_managed_skills_overview() -> Result<ManagedSkillsOverview, String> {
    let system_dir = get_system_skills_directory()?;
    let user_dir = get_user_skills_directory()?;

    let mut system_skills = scan_skills_internal(
        &system_dir,
        Some("global".to_string()),
        Some("system".to_string()),
    )
    .await?;
    let mut user_skills = scan_skills_internal(
        &user_dir,
        Some("global".to_string()),
        Some("user".to_string()),
    )
    .await?;
    let effective_skills = merge_skill_layers(vec![user_skills.clone(), system_skills.clone()]);

    system_skills.sort_by_cached_key(|skill| skill.name.to_lowercase());
    user_skills.sort_by_cached_key(|skill| skill.name.to_lowercase());

    Ok(ManagedSkillsOverview {
        system_directory: system_dir.to_string_lossy().to_string(),
        user_directory: user_dir.to_string_lossy().to_string(),
        system_skills,
        user_skills,
        effective_skills,
    })
}

pub async fn resolve_skills(
    system_dir: PathBuf,
    user_dir: PathBuf,
    assistant_dir: Option<PathBuf>,
    workspace_dir: Option<PathBuf>,
) -> Result<Vec<SkillMetadata>, String> {
    let mut skill_layers = Vec::new();

    let sources: Vec<(Option<PathBuf>, &str, &str)> = vec![
        (workspace_dir, "workspace", "workspace"),
        (assistant_dir, "assistant", "assistant"),
        (Some(user_dir), "global", "user"),
        (Some(system_dir), "global", "system"),
    ];

    for (directory, source, origin) in sources {
        let Some(directory) = directory else {
            continue;
        };

        let mut scanned = scan_skills_internal(
            &directory,
            Some(source.to_string()),
            Some(origin.to_string()),
        )
        .await?;
        scanned.sort_by_cached_key(|skill| skill.name.to_lowercase());
        skill_layers.push(scanned);
    }

    Ok(merge_skill_layers(skill_layers))
}

pub fn get_assistant_skills_directory(assistant_id: &str) -> Result<PathBuf, String> {
    let session_manager = get_session_manager()?;
    Ok(session_manager
        .get_base_data_dir()
        .join("assistants")
        .join(assistant_id)
        .join("skills"))
}

pub fn get_workspace_skills_directory_from_path(workspace_path: &Path) -> PathBuf {
    workspace_path.join("skills")
}

pub fn get_workspace_skills_directory_for_session(session_id: &str) -> Result<PathBuf, String> {
    let session_manager = get_session_manager()?;
    let workspace_dir = session_manager.get_session_workspace_dir_by_id(session_id);
    Ok(get_workspace_skills_directory_from_path(&workspace_dir))
}

pub fn collect_allowed_skill_roots(
    system_dir: PathBuf,
    user_dir: PathBuf,
    assistant_dir: Option<PathBuf>,
    workspace_dir: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(directory) = workspace_dir {
        roots.push(directory);
    }
    if let Some(directory) = assistant_dir {
        roots.push(directory);
    }
    roots.push(user_dir);
    roots.push(system_dir);

    roots
}

pub async fn resolve_skill_directories(
    assistant_id: Option<&str>,
    session_id: Option<&str>,
    workspace_path: Option<&Path>,
) -> Result<(PathBuf, PathBuf, Option<PathBuf>, Option<PathBuf>), String> {
    let system_dir = get_system_skills_directory()?;
    let user_dir = get_user_skills_directory()?;
    let assistant_dir = assistant_id
        .map(get_assistant_skills_directory)
        .transpose()?;
    let workspace_dir = if let Some(path) = workspace_path {
        Some(get_workspace_skills_directory_from_path(path))
    } else if let Some(id) = session_id {
        Some(get_workspace_skills_directory_for_session(id)?)
    } else {
        None
    };

    Ok((system_dir, user_dir, assistant_dir, workspace_dir))
}
