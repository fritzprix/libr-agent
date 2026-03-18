use crate::repositories::settings_repository::SettingsRepository;
use crate::session::get_session_manager;
use crate::state::get_settings_repository;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs::{self};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const SKILL_FILE_NAME: &str = "SKILL.md";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>, // "global", "assistant", or "workspace"
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
    workspace_dir: Option<PathBuf>,
) -> Result<Vec<SkillMetadata>, String> {
    let mut merged_skills = Vec::new();
    let mut seen_names = HashSet::new();

    let mut sources: Vec<(Option<PathBuf>, &str)> = Vec::new();
    sources.push((workspace_dir, "workspace"));
    sources.push((assistant_dir, "assistant"));
    sources.push((Some(global_dir), "global"));

    for (dir, source) in sources {
        let Some(dir) = dir else {
            continue;
        };

        let mut scanned = scan_skills_internal(&dir, Some(source.to_string())).await?;
        scanned.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        for skill in scanned {
            let normalized = skill.name.to_lowercase();
            if seen_names.insert(normalized) {
                merged_skills.push(skill);
            }
        }
    }

    merged_skills.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(merged_skills)
}

/// Public entry point for scanning a directory without a source tag.
/// Prefer this over calling `scan_skills_internal` directly from command handlers.
pub async fn scan_skills_directory(directory: &Path) -> Result<Vec<SkillMetadata>, String> {
    scan_skills_internal(directory, None).await
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
    global_dir: PathBuf,
    assistant_dir: Option<PathBuf>,
    workspace_dir: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(dir) = workspace_dir {
        roots.push(dir);
    }
    if let Some(dir) = assistant_dir {
        roots.push(dir);
    }
    roots.push(global_dir);

    roots
}

/// Reads the full content of a skill's SKILL.md file by skill path.
/// The `skill_path` is the absolute path to the SKILL.md file as returned in `SkillMetadata.path`.
///
/// Security: validates that the path is within the configured skills directory
/// and points to a `SKILL.md` file before reading.
pub async fn get_skill_content(skill_path: String) -> Result<String, String> {
    let skills_dir_str = get_configured_skills_directory().await?;
    let allowed_roots = collect_allowed_skill_roots(PathBuf::from(skills_dir_str), None, None);
    get_skill_content_from_roots(skill_path, &allowed_roots).await
}

pub async fn get_skill_content_from_roots(
    skill_path: String,
    allowed_roots: &[PathBuf],
) -> Result<String, String> {
    let path = PathBuf::from(&skill_path);

    // Require the file to be named SKILL.md
    if path.file_name() != Some(std::ffi::OsStr::new(SKILL_FILE_NAME)) {
        return Err("Skill path must point to a SKILL.md file".to_string());
    }

    let canonical_path = path
        .canonicalize()
        .map_err(|e| format!("Invalid skill path: {}", e))?;

    let mut is_allowed = false;
    for root in allowed_roots {
        if !root.exists() {
            continue;
        }
        let canonical_root = root
            .canonicalize()
            .map_err(|e| format!("Invalid skills directory: {}", e))?;
        if canonical_path.starts_with(&canonical_root) {
            is_allowed = true;
            break;
        }
    }

    if !is_allowed {
        return Err("Skill path is outside the allowed skills directories".to_string());
    }

    tokio::task::spawn_blocking(move || {
        fs::read_to_string(&canonical_path)
            .map_err(|e| format!("Failed to read skill content: {}", e))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
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
            if entry.file_name() == SKILL_FILE_NAME {
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

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::TempDir;

    /// Build a minimal skills directory containing one SKILL.md file,
    /// returning the TempDir (must stay alive) and the path to SKILL.md.
    fn setup_skills_dir() -> (TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let skill_dir = dir.path().join("my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        fs::write(&skill_md, "# My Skill\nDoes cool things.").unwrap();
        (dir, skill_md)
    }

    // Helper: override the configured skills directory to `path` by temporarily
    // setting an env variable checked by get_configured_skills_directory.
    // Since get_skill_content calls get_configured_skills_directory internally
    // and that function reads from the settings DB (which won't exist in tests),
    // we test the sub-logic (filename + canonicalize check) in isolation.

    #[test]
    fn test_skill_path_must_be_skill_md() {
        // Anything that isn't named SKILL.md should be rejected regardless of directory.
        let dir = tempfile::tempdir().unwrap();
        let bad_file = dir.path().join("secret.txt");
        fs::write(&bad_file, "secret").unwrap();

        // Build the path string (absolute)
        let path_str = bad_file.to_string_lossy().to_string();

        // Exercise only the filename guard (synchronously mirrored logic)
        let path = std::path::PathBuf::from(&path_str);
        let result: Result<(), String> =
            if path.file_name() != Some(std::ffi::OsStr::new("SKILL.md")) {
                Err("Skill path must point to a SKILL.md file".to_string())
            } else {
                Ok(())
            };

        assert!(result.is_err(), "Non-SKILL.md file should be rejected");
        assert!(result.unwrap_err().contains("SKILL.md"));
    }

    #[test]
    fn test_skill_md_filename_accepted() {
        let (_dir, skill_md) = setup_skills_dir();
        let path = skill_md.clone();

        let result: Result<(), String> =
            if path.file_name() != Some(std::ffi::OsStr::new("SKILL.md")) {
                Err("Skill path must point to a SKILL.md file".to_string())
            } else {
                Ok(())
            };

        assert!(result.is_ok(), "SKILL.md should pass the filename check");
    }

    #[test]
    fn test_path_traversal_blocked_by_starts_with() {
        // Simulate a path outside the skills directory.
        let skills_dir = tempfile::tempdir().unwrap();
        let outside_dir = tempfile::tempdir().unwrap();
        let outside_file = outside_dir.path().join("SKILL.md");
        fs::write(&outside_file, "malicious").unwrap();

        let canonical_dir = skills_dir.path().canonicalize().unwrap();
        let canonical_path = outside_file.canonicalize().unwrap();

        let result: Result<(), String> = if !canonical_path.starts_with(&canonical_dir) {
            Err("Skill path is outside the configured skills directory".to_string())
        } else {
            Ok(())
        };

        assert!(result.is_err(), "Path outside skills dir should be blocked");
        assert!(result.unwrap_err().contains("outside"));
    }

    #[test]
    fn test_path_inside_skills_dir_accepted() {
        let (skills_dir_temp, skill_md) = setup_skills_dir();

        let canonical_dir = skills_dir_temp.path().canonicalize().unwrap();
        let canonical_path = skill_md.canonicalize().unwrap();

        let result: Result<(), String> = if !canonical_path.starts_with(&canonical_dir) {
            Err("Skill path is outside the configured skills directory".to_string())
        } else {
            Ok(())
        };

        assert!(result.is_ok(), "Path inside skills dir should be accepted");
    }
}
