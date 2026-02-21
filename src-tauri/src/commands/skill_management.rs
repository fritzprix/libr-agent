use crate::session::get_session_manager;
use log::{info, warn};
use std::fs::{self};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Recursively copies contents of `src` directory into `dst`.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if !dst.exists() {
        fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    }

    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn copy_global_to_assistant(
    assistant_id: String,
    skill_name: String,
) -> Result<String, String> {
    let global_dir_str = crate::commands::skill_commands::get_configured_skills_directory().await?;
    let global_skill_path = PathBuf::from(global_dir_str).join(&skill_name);

    if !global_skill_path.exists() {
        return Err(format!("Global skill '{}' not found", skill_name));
    }

    let session_manager = get_session_manager()?;
    let assistant_skills_dir = session_manager
        .get_base_data_dir()
        .join("assistants")
        .join(&assistant_id)
        .join("skills");
    let target_path = assistant_skills_dir.join(&skill_name);

    if target_path.exists() {
        return Err(format!(
            "Skill '{}' already exists for this assistant",
            skill_name
        ));
    }

    // Copy recursively
    copy_dir_recursive(&global_skill_path, &target_path)?;

    Ok(format!(
        "Successfully copied skill '{}' to assistant '{}'",
        skill_name, assistant_id
    ))
}

#[tauri::command]
pub async fn delete_assistant_skill(
    assistant_id: String,
    skill_name: String,
) -> Result<String, String> {
    let session_manager = get_session_manager()?;
    let assistant_skills_dir = session_manager
        .get_base_data_dir()
        .join("assistants")
        .join(&assistant_id)
        .join("skills");
    let target_path = assistant_skills_dir.join(&skill_name);

    if !target_path.exists() {
        return Err(format!("Assistant skill '{}' not found", skill_name));
    }

    fs::remove_dir_all(&target_path).map_err(|e| e.to_string())?;

    Ok(format!(
        "Successfully deleted assistant skill '{}'",
        skill_name
    ))
}
#[tauri::command]
pub async fn reset_assistant_skills(assistant_id: String) -> Result<String, String> {
    let session_manager = get_session_manager()?;
    let assistant_skills_dir = session_manager
        .get_base_data_dir()
        .join("assistants")
        .join(&assistant_id)
        .join("skills");

    if assistant_skills_dir.exists() {
        fs::remove_dir_all(&assistant_skills_dir).map_err(|e| e.to_string())?;
        Ok(format!(
            "Successfully reset skills for assistant '{}'",
            assistant_id
        ))
    } else {
        Ok("No assistant skills to reset".to_string())
    }
}

#[tauri::command]
pub async fn import_assistant_skills(
    assistant_id: String,
    file_path: String,
) -> Result<String, String> {
    let session_manager = get_session_manager()?;
    let assistant_skills_dir = session_manager
        .get_base_data_dir()
        .join("assistants")
        .join(&assistant_id)
        .join("skills");

    // Ensure assistant skills directory exists
    if !assistant_skills_dir.exists() {
        fs::create_dir_all(&assistant_skills_dir).map_err(|e| e.to_string())?;
    }

    // Use a temp directory for extraction/copying
    let temp_dir = session_manager
        .get_base_data_dir()
        .join("temp_import_skills");

    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

    let src_path = PathBuf::from(&file_path);
    if !src_path.exists() {
        return Err(format!("Source path does not exist: {}", file_path));
    }

    // 1. Extract/Copy to Temp
    if src_path.is_file() {
        if let Some(ext) = src_path.extension() {
            if ext == "zip" {
                let file = fs::File::open(&src_path).map_err(|e| e.to_string())?;
                let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
                archive.extract(&temp_dir).map_err(|e| e.to_string())?;
            } else {
                return Err("Only .zip files or directories are supported".to_string());
            }
        } else {
            return Err("Invalid file type".to_string());
        }
    } else if src_path.is_dir() {
        // Copy directory contents to temp
        copy_dir_recursive(&src_path, &temp_dir)?;
    } else {
        return Err("Invalid source path".to_string());
    }

    // 2. Scan for Skill Roots in Temp: find folders containing SKILL.md
    info!(
        "Scanning for skill roots (SKILL.md) in import temp dir: {:?}",
        temp_dir
    );
    let mut skill_roots = Vec::new();
    for entry in WalkDir::new(&temp_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_name() == "SKILL.md" {
            if let Some(parent) = entry.path().parent() {
                skill_roots.push(parent.to_path_buf());
            }
        }
    }

    if skill_roots.is_empty() {
        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
        return Err("No skills (SKILL.md) found in the imported files".to_string());
    }

    // 3. Move/Install Skills to Assistant Directory
    let mut imported_count = 0;
    for root in skill_roots {
        if let Some(folder_name) = root.file_name() {
            let target_path = assistant_skills_dir.join(folder_name);
            info!("Importing skill '{:?}' to {:?}", folder_name, target_path);

            // Remove existing skill if it exists (overwrite)
            if target_path.exists() {
                fs::remove_dir_all(&target_path).map_err(|e| e.to_string())?;
            }

            // Move (rename) or Copy
            if let Err(e) = fs::rename(&root, &target_path) {
                warn!(
                    "Failed to move {:?} to {:?} (error: {}), attempting recursive copy...",
                    root, target_path, e
                );
                copy_dir_recursive(&root, &target_path)?;
            }
            imported_count += 1;
        }
    }

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);

    Ok(format!("Successfully imported {} skills", imported_count))
}
