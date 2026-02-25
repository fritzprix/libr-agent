use crate::repositories::settings_repository::SettingsRepository;
use crate::session::get_session_manager;
use crate::state::get_settings_repository;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>, // "global" or "assistant"
}

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: String,
}

pub async fn get_default_skills_directory() -> Result<String, String> {
    let session_manager = get_session_manager()?;
    let skills_dir = session_manager.get_base_data_dir().join("skills");
    Ok(skills_dir.to_string_lossy().to_string())
}

pub async fn get_configured_skills_directory() -> Result<String, String> {
    let repo = get_settings_repository();

    match repo.get("systemSettings").await {
        Ok(Some(model)) => match serde_json::from_str::<Value>(&model.value) {
            Ok(json) => {
                if let Some(skills_dir) = json.get("skillsDirectory").and_then(|v| v.as_str()) {
                    if !skills_dir.is_empty() {
                        return Ok(skills_dir.to_string());
                    }
                }
            }
            Err(e) => {
                warn!("Failed to parse systemSettings JSON: {}", e);
            }
        },
        Err(e) => {
            warn!("Failed to get systemSettings from repository: {}", e);
        }
        Ok(None) => {}
    }

    // Fallback to default
    get_default_skills_directory().await
}

pub async fn resolve_skills(
    global_dir: PathBuf,
    assistant_dir: Option<PathBuf>,
) -> Result<Vec<SkillMetadata>, String> {
    // 1. Check if assistant has skills
    if let Some(dir) = assistant_dir {
        if dir.exists() {
            let assistant_skills =
                scan_skills_internal(&dir, Some("assistant".to_string())).await?;

            // If assistant has any skills, return ONLY those (no global skills)
            if !assistant_skills.is_empty() {
                let mut skills = assistant_skills;
                skills.sort_by(|a, b| a.name.cmp(&b.name));
                return Ok(skills);
            }
        }
    }

    // 2. Fallback: Return global skills if no assistant skills exist
    let mut global_skills = scan_skills_internal(&global_dir, Some("global".to_string())).await?;
    global_skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(global_skills)
}

/// Public entry point for scanning a directory without a source tag.
/// Prefer this over calling `scan_skills_internal` directly from command handlers.
pub async fn scan_skills_directory(directory: &Path) -> Result<Vec<SkillMetadata>, String> {
    scan_skills_internal(directory, None).await
}

pub(crate) async fn scan_skills_internal(
    root_path: &Path,
    source_tag: Option<String>,
) -> Result<Vec<SkillMetadata>, String> {
    if !root_path.exists() {
        info!("Skills directory does not exist: {:?}", root_path);
        return Ok(Vec::new());
    }

    let root_path_owned = root_path.to_owned();
    let source_tag_owned = source_tag.clone();

    tokio::task::spawn_blocking(move || {
        let mut skills = Vec::new();

        for entry in WalkDir::new(&root_path_owned)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == "SKILL.md" {
                let path = entry.path();
                match parse_skill_metadata(path) {
                    Ok(mut metadata) => {
                        metadata.source = source_tag_owned.clone();
                        skills.push(metadata);
                    }
                    Err(e) => {
                        warn!("Failed to parse skill at {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(skills)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

pub fn parse_skill_metadata(path: &Path) -> Result<SkillMetadata, String> {
    let content = fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::InvalidData {
            "Content appears to be binary or contains invalid UTF-8 characters".to_string()
        } else {
            e.to_string()
        }
    })?;

    // Simple frontmatter parsing
    if let Some(stripped) = content.strip_prefix("---") {
        if let Some(end_idx) = stripped.find("---") {
            let frontmatter_str = &stripped[..end_idx];
            let frontmatter: SkillFrontmatter = serde_yaml::from_str(frontmatter_str)
                .map_err(|e| format!("YAML parse error: {}", e))?;

            return Ok(SkillMetadata {
                name: frontmatter.name,
                description: frontmatter.description,
                path: path.to_string_lossy().to_string(),
                source: None,
            });
        }
    }

    Err("No valid YAML frontmatter found".to_string())
}

/// Recursively copies contents of `src` directory into `dst`.
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
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

pub async fn copy_global_to_assistant(
    assistant_id: String,
    skill_name: String,
) -> Result<String, String> {
    let global_dir_str = get_configured_skills_directory().await?;
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
    let temp_dir = session_manager
        .get_base_data_dir()
        .join("temp_import_skills");

    tokio::task::spawn_blocking(move || {
        import_assistant_skills_blocking(assistant_skills_dir, temp_dir, file_path)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Synchronous implementation of skill import, safe to run inside spawn_blocking.
fn import_assistant_skills_blocking(
    assistant_skills_dir: PathBuf,
    temp_dir: PathBuf,
    file_path: String,
) -> Result<String, String> {
    // Ensure assistant skills directory exists
    if !assistant_skills_dir.exists() {
        fs::create_dir_all(&assistant_skills_dir).map_err(|e| e.to_string())?;
    }

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
                // Use secure extraction to prevent Zip Slip vulnerability
                crate::utils::fs::extract_zip_secure(&mut archive, &temp_dir)?;
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
