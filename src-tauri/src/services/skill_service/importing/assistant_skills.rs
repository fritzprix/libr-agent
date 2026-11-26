use super::super::cache::invalidate_skill_scan_cache;
use super::super::contracts::PLUGIN_SKILL_OWNER_FILE;
use super::super::directories::{
    get_assistant_skills_directory, get_system_skills_directory, get_user_skills_directory,
    resolve_skills,
};
use super::super::scanning::{
    copy_dir_recursive, find_skill_root_by_name, plugin_owner_marker_of_dir,
    remove_skill_name_from_directory, skill_storage_directory_name,
};
use super::archive::prepare_local_skill_import;
use super::user_install::{
    install_prepared_skills_to_directory, invalidate_skill_cache_after_success,
};
use std::fs;
use std::path::{Path, PathBuf};

pub async fn delete_user_skill(skill_name: String) -> Result<String, String> {
    let user_dir = get_user_skills_directory()?;
    // #2927 D2: a plugin-owned skill is read-only for the user. It goes only
    // with its plugin, so a lone delete is rejected with the owner named.
    if let Some(root) = find_skill_root_by_name(&user_dir, &skill_name)? {
        if let Some(owner) = plugin_owner_marker_of_dir(&root) {
            return Err(format!(
                "Skill '{skill_name}' is owned by plugin {owner}. Disable or uninstall the \
                 plugin to remove it."
            ));
        }
    }
    invalidate_skill_cache_after_success(remove_skill_name_from_directory(&user_dir, &skill_name))
        .map(|_| format!("Successfully deleted user skill '{}'", skill_name))
}

pub async fn reset_user_skills() -> Result<String, String> {
    let user_dir = get_user_skills_directory()?;
    if user_dir.exists() {
        invalidate_skill_cache_after_success(
            fs::remove_dir_all(&user_dir).map_err(|error| error.to_string()),
        )?;
        Ok("Successfully reset user skills".to_string())
    } else {
        Ok("No user skills to reset".to_string())
    }
}

pub async fn copy_global_to_assistant(
    assistant_id: String,
    skill_name: String,
) -> Result<String, String> {
    let system_dir = get_system_skills_directory()?;
    let user_dir = get_user_skills_directory()?;
    let global_skills = resolve_skills(system_dir, user_dir, None, None, None).await?;
    let source_skill = global_skills
        .into_iter()
        .find(|skill| skill.name.eq_ignore_ascii_case(&skill_name))
        .ok_or_else(|| format!("Global skill '{}' not found", skill_name))?;

    let source_root = PathBuf::from(&source_skill.path)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("Failed to resolve root directory for '{}'", skill_name))?;

    let assistant_skills_dir = get_assistant_skills_directory(&assistant_id)?;
    if find_skill_root_by_name(&assistant_skills_dir, &skill_name)?.is_some() {
        return Err(format!(
            "Skill '{}' already exists for this assistant",
            skill_name
        ));
    }

    let storage_name = skill_storage_directory_name(&source_skill.name)?;
    let target_path = assistant_skills_dir.join(storage_name);
    invalidate_skill_cache_after_success(copy_dir_recursive(&source_root, &target_path))?;
    // #2927 D2: a copy into an assistant is the user's own skill; the
    // plugin-owner marker must not travel with it, or the assistant copy
    // would inherit the plugin's read-only rule. A missing marker (plain
    // skill) is fine; any other failure would leave an undeletable
    // assistant copy, so it is reported and the copy is rolled back
    // (#2954 G1.4).
    if let Err(error) = fs::remove_file(target_path.join(PLUGIN_SKILL_OWNER_FILE)) {
        if error.kind() != std::io::ErrorKind::NotFound {
            let _ = fs::remove_dir_all(&target_path);
            invalidate_skill_scan_cache();
            return Err(format!(
                "strip plugin-owner marker from assistant copy of '{skill_name}': {error}"
            ));
        }
    }

    Ok(format!(
        "Successfully copied skill '{}' to assistant '{}'",
        skill_name, assistant_id
    ))
}

pub async fn delete_assistant_skill(
    assistant_id: String,
    skill_name: String,
) -> Result<String, String> {
    let assistant_skills_dir = get_assistant_skills_directory(&assistant_id)?;
    invalidate_skill_cache_after_success(remove_skill_name_from_directory(
        &assistant_skills_dir,
        &skill_name,
    ))?;

    Ok(format!(
        "Successfully deleted assistant skill '{}'",
        skill_name
    ))
}

pub async fn reset_assistant_skills(assistant_id: String) -> Result<String, String> {
    let assistant_skills_dir = get_assistant_skills_directory(&assistant_id)?;

    if assistant_skills_dir.exists() {
        invalidate_skill_cache_after_success(
            fs::remove_dir_all(&assistant_skills_dir).map_err(|error| error.to_string()),
        )?;
        Ok(format!(
            "Successfully reset skills for assistant '{}'",
            assistant_id
        ))
    } else {
        Ok("No assistant skills to reset".to_string())
    }
}

pub async fn import_assistant_skills(
    assistant_id: String,
    file_path: String,
) -> Result<String, String> {
    let assistant_skills_dir = get_assistant_skills_directory(&assistant_id)?;
    let prepared = prepare_local_skill_import(file_path).await?;
    let result = install_prepared_skills_to_directory(&prepared, assistant_skills_dir, true, None)?;
    Ok(format!(
        "Successfully imported {} skills",
        result.imported_names.len()
    ))
}
