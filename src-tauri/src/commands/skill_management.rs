use crate::session::get_session_manager;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use log::{error, info, warn};
use std::fs::{self};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[tauri::command]
pub async fn download_global_skills() -> Result<String, String> {
    let repo_url = "https://github.com/fritzprix/skills/archive/refs/heads/main.zip";
    info!("Starting download_global_skills from {}", repo_url);

    let session_manager = get_session_manager().map_err(|e| {
        error!("Failed to get session manager: {}", e);
        e
    })?;

    let app_cache_dir = session_manager.get_base_data_dir(); // This usually points to app data, maybe better to use a cache dir or temp dir for download
    let temp_dir = app_cache_dir.join("temp_skills_download");
    let global_skills_dir = app_cache_dir.join("skills");

    info!(
        "Directories - Temp: {:?}, Global: {:?}",
        temp_dir, global_skills_dir
    );

    // 1. Download the repo with progress bar
    info!("Initiating HTTP GET request...");
    let response = reqwest::get(repo_url).await.map_err(|e| {
        error!("Failed to download skills: {}", e);
        e.to_string()
    })?;

    if !response.status().is_success() {
        let status = response.status();
        error!("Download failed with HTTP status: {}", status);
        return Err(format!("Download failed with status: {}", status));
    }

    let total_size = response.content_length().unwrap_or(0);
    info!("Starting download stream. Total size: {} bytes", total_size);

    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .progress_chars("#>-"));

    let mut stream = response.bytes_stream();
    let mut content = Vec::new();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| {
            error!("Error while streaming download: {}", e);
            e.to_string()
        })?;
        content.extend_from_slice(&chunk);
        pb.inc(chunk.len() as u64);
    }

    pb.finish_with_message("Download complete");
    info!("Download complete. Received {} bytes", content.len());

    // 2. Extract to temp directory
    if temp_dir.exists() {
        info!("Cleaning up existing temp directory: {:?}", temp_dir);
        fs::remove_dir_all(&temp_dir).map_err(|e| {
            error!("Failed to remove temp dir: {}", e);
            e.to_string()
        })?;
    }
    fs::create_dir_all(&temp_dir).map_err(|e| {
        error!("Failed to create temp dir: {}", e);
        e.to_string()
    })?;

    info!("Extracting zip archive...");
    let reader = Cursor::new(content);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| {
        error!("Failed to read zip archive: {}", e);
        e.to_string()
    })?;

    // Log extraction
    for i in 0..archive.len() {
        let _file = archive.by_index(i).map_err(|e| e.to_string())?;
        // trace!("Archive file: {}", file.name()); // Trace might be too verbose
    }

    archive.extract(&temp_dir).map_err(|e| {
        error!("Failed to extract archive: {}", e);
        e.to_string()
    })?;
    info!("Extraction complete.");

    // 3. Walk and find Skill Roots
    info!("Scanning for skill roots (SKILL.md) in {:?}", temp_dir);
    let mut skill_roots = Vec::new();
    for entry in WalkDir::new(&temp_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_name() == "SKILL.md" {
            if let Some(parent) = entry.path().parent() {
                info!("Found skill root: {:?}", parent);
                skill_roots.push(parent.to_path_buf());
            }
        }
    }

    if skill_roots.is_empty() {
        warn!(
            "No skills (SKILL.md) found in the downloaded repository at {:?}",
            temp_dir
        );
        return Err("No skills found in the downloaded repository".to_string());
    }

    // 4. Move Skill Roots to Global Skills Directory
    if !global_skills_dir.exists() {
        info!("Creating global skills directory: {:?}", global_skills_dir);
        fs::create_dir_all(&global_skills_dir).map_err(|e| {
            error!("Failed to create global skills dir: {}", e);
            e.to_string()
        })?;
    }

    let mut installed_count = 0;
    for root in skill_roots {
        if let Some(folder_name) = root.file_name() {
            let target_path = global_skills_dir.join(folder_name);
            info!("Installing skill '{:?}' to {:?}", folder_name, target_path);

            // If target exists, merge or overwrite? For now, let's remove and replace to ensure clean update
            if target_path.exists() {
                info!("Removing existing skill at {:?}", target_path);
                fs::remove_dir_all(&target_path).map_err(|e| {
                    error!("Failed to remove existing skill: {}", e);
                    e.to_string()
                })?;
            }

            // Move (rename)
            // Rename might fail across filesystems (e.g. temp is /tmp and appdata is /home), so copy-recursively is safer if rename fails, but crate::utils::fs::copy_dir_all might exist or we can use fs_extra
            // For simplicity in this environment, let's try fs::rename first, fallback to copy
            if let Err(e) = fs::rename(&root, &target_path) {
                // Fallback: This requires a recursive copy helper.
                // Assuming we can use a helper or just error out for now.
                // Actually, let's use a simple recursive copy implementation if needed or just error.
                // Better: use fs_extra if available, or just walk and copy.
                warn!(
                    "Failed to rename {:?} to {:?} (error: {}), attempting recursive copy...",
                    root, target_path, e
                );
                copy_dir_recursive(&root, &target_path).map_err(|e| {
                    error!("Failed to copy skill: {}", e);
                    e
                })?;
            }
            installed_count += 1;
        }
    }

    // Cleanup
    info!("Cleaning up temp directory...");
    let _ = fs::remove_dir_all(&temp_dir);

    info!("Successfully installed {} skills", installed_count);
    Ok(format!("Successfully installed {} skills", installed_count))
}

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

    // 2. Scan for Skill Roots in Temp
    // Logic similar to download_global_skills: find folders containing SKILL.md
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
