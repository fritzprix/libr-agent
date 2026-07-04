use super::contracts::{
    ManagedSkillsOverview, SkillAliasRoot, SkillMetadata, ASSISTANT_SKILLS_ALIAS_PREFIX,
    LEGACY_SYSTEM_SKILLS_DIR_NAME, SYSTEM_SKILLS_ALIAS_PREFIX, SYSTEM_SKILLS_DIR_NAME,
    USER_SKILLS_ALIAS_PREFIX, USER_SKILLS_DIR_NAME, WORKSPACE_SKILLS_ALIAS_PREFIX,
};
use super::copy_dir_recursive;
use super::scan_skills_internal_cached;
use crate::repositories::settings_repository::SettingsRepository;
use crate::session::get_session_manager;
use crate::state::{try_get_settings_repository, wait_for_managed_skills_sync};
use log;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub async fn get_default_skills_directory() -> Result<String, String> {
    Ok(get_user_skills_directory()?.to_string_lossy().to_string())
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

pub fn build_skill_alias_path(
    root: &Path,
    skill_path: &Path,
    alias_prefix: &str,
) -> Option<String> {
    let relative = skill_path.strip_prefix(root).ok()?;
    let relative = relative.to_string_lossy().replace('\\', "/");

    if relative.is_empty() {
        Some(alias_prefix.to_string())
    } else {
        Some(format!("{alias_prefix}/{relative}"))
    }
}

pub fn extract_skill_alias_relative_path(path_str: &str) -> Option<(&'static str, &str)> {
    for prefix in [
        SYSTEM_SKILLS_ALIAS_PREFIX,
        USER_SKILLS_ALIAS_PREFIX,
        ASSISTANT_SKILLS_ALIAS_PREFIX,
        WORKSPACE_SKILLS_ALIAS_PREFIX,
    ] {
        if path_str == prefix {
            return Some((prefix, "."));
        }

        if let Some(suffix) = path_str
            .strip_prefix(&format!("{prefix}/"))
            .or_else(|| path_str.strip_prefix(&format!("{prefix}\\")))
        {
            return Some((
                prefix,
                if suffix.trim().is_empty() {
                    "."
                } else {
                    suffix
                },
            ));
        }
    }

    None
}

pub fn collect_skill_alias_roots(
    system_dir: PathBuf,
    user_dir: PathBuf,
    assistant_dir: Option<PathBuf>,
    workspace_dir: Option<PathBuf>,
) -> Vec<SkillAliasRoot> {
    let mut roots = vec![
        SkillAliasRoot {
            prefix: SYSTEM_SKILLS_ALIAS_PREFIX,
            root: system_dir,
        },
        SkillAliasRoot {
            prefix: USER_SKILLS_ALIAS_PREFIX,
            root: user_dir,
        },
    ];

    if let Some(root) = assistant_dir {
        roots.push(SkillAliasRoot {
            prefix: ASSISTANT_SKILLS_ALIAS_PREFIX,
            root,
        });
    }

    if let Some(root) = workspace_dir {
        roots.push(SkillAliasRoot {
            prefix: WORKSPACE_SKILLS_ALIAS_PREFIX,
            root,
        });
    }

    roots
}

fn apply_skill_alias_paths(skills: &mut [SkillMetadata], root: &Path, alias_prefix: &str) {
    for skill in skills {
        skill.alias_path = build_skill_alias_path(root, Path::new(&skill.path), alias_prefix);
    }
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

const AGENT_SKILL_PATTERNS: &[&str] = &[
    ".agents/skills",
    ".gemini/skills",
    ".copilot/skills",
    ".cursor/skills",
    ".windsurf/skills",
    ".claude/skills",
    ".cline/skills",
    ".continue/skills",
];

fn find_workspace_root(workspace_dir: &Path) -> Option<PathBuf> {
    // Agent skill auto-discovery only applies to the canonical workspace layout:
    // `<project>/.libragent/skills`. Do not walk up to arbitrary `.git` roots —
    // integration tests (and temp workspaces under the LibrAgent repo tree) would
    // otherwise inherit this repository's `.agents/skills` directory.
    if workspace_dir.ends_with(".libragent/skills") {
        return workspace_dir
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf());
    }

    None
}

fn discover_agent_skill_dirs(workspace_root: &Path) -> Vec<PathBuf> {
    AGENT_SKILL_PATTERNS
        .iter()
        .map(|p| workspace_root.join(p))
        .filter(|p| p.exists() && p.is_dir())
        .collect()
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
    apply_skill_alias_paths(&mut system_skills, &system_dir, SYSTEM_SKILLS_ALIAS_PREFIX);
    let mut user_skills = scan_skills_internal_cached(
        &user_dir,
        Some("global".to_string()),
        Some("user".to_string()),
    )
    .await?;
    apply_skill_alias_paths(&mut user_skills, &user_dir, USER_SKILLS_ALIAS_PREFIX);
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
    let mut sources: Vec<(Option<PathBuf>, &str, &str, Option<&'static str>)> = vec![(
        workspace_dir.clone(),
        "workspace",
        "workspace",
        Some(WORKSPACE_SKILLS_ALIAS_PREFIX),
    )];

    // Auto-discover agent hidden directories
    let workspace_root = workspace_dir.as_ref().and_then(|d| find_workspace_root(d));
    if let Some(ws) = workspace_root {
        for path in discover_agent_skill_dirs(&ws) {
            log::info!("Auto-discovered agent skills directory: {:?}", path);
            sources.push((Some(path), "agent_import", "agent", None));
        }
    }

    sources.push((
        assistant_dir,
        "assistant",
        "assistant",
        Some(ASSISTANT_SKILLS_ALIAS_PREFIX),
    ));

    // Read additional skill paths from the settings repository and dynamically merge reference layers
    if let Some(settings_repo) = try_get_settings_repository() {
        if let Ok(Some(setting)) = settings_repo.get("additionalSkillPaths").await {
            if let Ok(paths) = serde_json::from_str::<Vec<String>>(&setting.value) {
                let valid_paths = paths
                    .into_iter()
                    .map(PathBuf::from)
                    .filter(|p| p.exists() && p.is_dir());
                for path in valid_paths {
                    sources.push((Some(path), "custom_reference", "custom", None));
                }
            }
        }
    }

    sources.push((
        Some(user_dir),
        "global",
        "user",
        Some(USER_SKILLS_ALIAS_PREFIX),
    ));
    sources.push((
        Some(system_dir),
        "global",
        "system",
        Some(SYSTEM_SKILLS_ALIAS_PREFIX),
    ));

    for (directory, source, origin, alias_prefix) in sources {
        let Some(directory) = directory else {
            continue;
        };

        let mut scanned = scan_skills_internal_cached(
            &directory,
            Some(source.to_string()),
            Some(origin.to_string()),
        )
        .await?;
        if let Some(alias_prefix) = alias_prefix {
            apply_skill_alias_paths(&mut scanned, &directory, alias_prefix);
        }
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

        // 1. Try fs::rename first (atomic operation if on same filesystem)
        if fs::rename(&old_skills, &new_skills).is_ok() {
            log::info!("Successfully migrated workspace skills via rename");
            return Ok(());
        }

        // 2. Fallback: copy then remove if rename fails
        copy_dir_recursive(&old_skills, &new_skills)
            .map_err(|e| format!("Skills migration copy failed: {}", e))?;

        fs::remove_dir_all(&old_skills)
            .map_err(|e| format!("Failed to remove legacy skills folder: {}", e))?;
        log::info!("Successfully migrated workspace skills to .libragent/skills (fallback)");
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

    let workspace_root = workspace_dir.as_ref().and_then(|d| find_workspace_root(d));

    if let Some(directory) = workspace_dir {
        roots.push(directory);
    }

    // Add agent skill directories
    if let Some(ws) = workspace_root {
        for path in discover_agent_skill_dirs(&ws) {
            roots.push(path);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn find_workspace_root_returns_none_for_noncanonical_workspace_dirs() {
        let temp = TempDir::new().expect("temp dir");
        let nested = temp.path().join("dir1").join("dir2");
        fs::create_dir_all(&nested).expect("nested dir");

        assert!(find_workspace_root(&nested).is_none());
    }

    #[test]
    fn find_workspace_root_does_not_walk_to_git_root_from_temp_workspace() {
        let temp = TempDir::new().expect("temp dir");
        let workspace_dir = temp.path().join("workspace-skills");
        fs::create_dir_all(&workspace_dir).expect("workspace dir");

        assert!(find_workspace_root(&workspace_dir).is_none());
    }

    #[test]
    fn find_workspace_root_resolves_parent_for_libragent_skills_layout() {
        let temp = TempDir::new().expect("temp dir");
        let workspace_dir = temp.path().join(".libragent").join("skills");
        fs::create_dir_all(&workspace_dir).expect("workspace skills dir");

        assert_eq!(
            find_workspace_root(&workspace_dir),
            Some(temp.path().to_path_buf())
        );
    }
}
