use super::contracts::{
    ManagedSkillsOverview, SkillMetadata, LEGACY_SYSTEM_SKILLS_DIR_NAME, SYSTEM_SKILLS_DIR_NAME,
    USER_SKILLS_DIR_NAME,
};
use super::copy_dir_recursive;
use super::scan_skills_internal_cached;
use crate::repositories::settings_repository::SettingsRepository;
use crate::session::get_session_manager;
use crate::state::{get_settings_repository, wait_for_managed_skills_sync};
use log;
use std::collections::HashSet;
use std::fs;
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

    get_managed_skills_overview_for_directories(system_dir, user_dir).await
}

pub async fn get_managed_skills_overview_for_directories(
    system_dir: PathBuf,
    user_dir: PathBuf,
) -> Result<ManagedSkillsOverview, String> {
    wait_for_managed_skills_sync().await;

    let mut system_skills = scan_skills_internal_cached(
        &system_dir,
        Some("global".to_string()),
        Some("system".to_string()),
    )
    .await?;
    let mut user_skills = scan_skills_internal_cached(
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
    wait_for_managed_skills_sync().await;

    let mut skill_layers = Vec::new();
    let mut sources: Vec<(Option<PathBuf>, &str, &str)> = vec![
        (workspace_dir, "workspace", "workspace"),
        (assistant_dir, "assistant", "assistant"),
    ];

    // 설정 저장소에서 추가 스킬 경로(additionalSkillPaths)를 읽어와 동적으로 레퍼런스 레이어 병합
    let settings_repo = get_settings_repository();
    if let Ok(Some(setting)) = settings_repo.get("additionalSkillPaths").await {
        if let Ok(paths) = serde_json::from_str::<Vec<String>>(&setting.value) {
            let valid_paths = paths
                .into_iter()
                .map(PathBuf::from)
                .filter(|p| p.exists() && p.is_dir());
            for path in valid_paths {
                sources.push((Some(path), "custom_reference", "custom"));
            }
        }
    }

    sources.push((Some(user_dir), "global", "user"));
    sources.push((Some(system_dir), "global", "system"));

    for (directory, source, origin) in sources {
        let Some(directory) = directory else {
            continue;
        };

        let mut scanned = scan_skills_internal_cached(
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
    workspace_path.join(".libragent").join("skills")
}

pub fn get_workspace_skills_directory_for_session(session_id: &str) -> Result<PathBuf, String> {
    let session_manager = get_session_manager()?;
    let workspace_dir = session_manager.get_session_workspace_dir_by_id(session_id);
    Ok(get_workspace_skills_directory_from_path(&workspace_dir))
}

pub fn migrate_workspace_skills_to_libragent(workspace_path: &Path) -> Result<(), String> {
    let old_skills = workspace_path.join("skills");
    let new_skills = get_workspace_skills_directory_from_path(workspace_path);

    if old_skills.exists() && !new_skills.exists() {
        let parent = new_skills
            .parent()
            .ok_or_else(|| "Failed to resolve .libragent parent directory".to_string())?;
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create .libragent folder: {}", e))?;

        // 1. fs::rename 우선 시도 (동일 파일 시스템 시 원자적 처리)
        if fs::rename(&old_skills, &new_skills).is_ok() {
            log::info!("Successfully migrated workspace skills via rename");
            return Ok(());
        }

        // 2. rename 실패 시 복사 + 삭제 폴백
        copy_dir_recursive(&old_skills, &new_skills)
            .map_err(|e| format!("Skills migration copy failed: {}", e))?;

        if new_skills.exists() {
            fs::remove_dir_all(&old_skills)
                .map_err(|e| format!("Failed to remove legacy skills folder: {}", e))?;
            log::info!("Successfully migrated workspace skills to .libragent/skills (fallback)");
        }
    }

    Ok(())
}

pub fn initialize_workspace_skills(workspace_path: &Path) -> Result<(), String> {
    migrate_workspace_skills_to_libragent(workspace_path)?;

    let skills_dir = get_workspace_skills_directory_from_path(workspace_path);
    if !skills_dir.exists() {
        fs::create_dir_all(&skills_dir)
            .map_err(|e| format!("Failed to create workspace skills directory: {}", e))?;
    }

    Ok(())
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
        initialize_workspace_skills(path)?;
        Some(get_workspace_skills_directory_from_path(path))
    } else if let Some(id) = session_id {
        let session_manager = get_session_manager()?;
        let workspace_path = session_manager.get_session_workspace_dir_by_id(id);
        initialize_workspace_skills(&workspace_path)?;
        Some(get_workspace_skills_directory_from_path(&workspace_path))
    } else {
        None
    };

    Ok((system_dir, user_dir, assistant_dir, workspace_dir))
}
