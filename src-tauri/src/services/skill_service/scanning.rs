use super::contracts::{SkillFrontmatter, SkillMetadata, SKILL_FILE_NAME};
use super::directories::{collect_allowed_skill_roots, resolve_skill_directories};
use log::{info, warn};
use std::ffi::OsStr;
use std::fs::{self};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub async fn scan_skills_directory(directory: &Path) -> Result<Vec<SkillMetadata>, String> {
    scan_skills_internal(directory, None, None).await
}

pub(crate) async fn scan_skills_internal(
    root_path: &Path,
    source_tag: Option<String>,
    origin_tag: Option<String>,
) -> Result<Vec<SkillMetadata>, String> {
    if !root_path.exists() {
        info!("Skills directory does not exist: {:?}", root_path);
        return Ok(Vec::new());
    }

    let root_path_owned = root_path.to_owned();
    let source_tag_owned = source_tag.clone();
    let origin_tag_owned = origin_tag.clone();

    tokio::task::spawn_blocking(move || {
        let mut skills = Vec::new();

        for entry in WalkDir::new(&root_path_owned)
            .follow_links(false)
            .into_iter()
            .filter_map(|entry| entry.ok())
        {
            if entry.file_name() == SKILL_FILE_NAME {
                let path = entry.path();
                match parse_skill_metadata(path) {
                    Ok(mut metadata) => {
                        metadata.source = source_tag_owned.clone();
                        metadata.origin = origin_tag_owned.clone();
                        skills.push(metadata);
                    }
                    Err(error) => {
                        warn!("Failed to parse skill at {:?}: {}", path, error);
                    }
                }
            }
        }
        Ok(skills)
    })
    .await
    .map_err(|error| format!("Task join error: {}", error))?
}

pub fn parse_skill_metadata(path: &Path) -> Result<SkillMetadata, String> {
    let content = fs::read_to_string(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            "Content appears to be binary or contains invalid UTF-8 characters".to_string()
        } else {
            error.to_string()
        }
    })?;
    let content = content.strip_prefix('\u{feff}').unwrap_or(&content);

    if let Some(stripped) = content.strip_prefix("---") {
        if let Some(end_idx) = stripped.find("---") {
            let frontmatter_str = &stripped[..end_idx];
            let frontmatter: SkillFrontmatter = serde_yaml::from_str(frontmatter_str)
                .map_err(|error| format!("YAML parse error: {}", error))?;

            if frontmatter.name.trim().is_empty() {
                return Err("Skill name cannot be empty".to_string());
            }
            if frontmatter.description.trim().is_empty() {
                return Err("Skill description cannot be empty".to_string());
            }

            return Ok(SkillMetadata {
                name: frontmatter.name,
                description: frontmatter.description,
                path: path.to_string_lossy().to_string(),
                source: None,
                origin: None,
            });
        }
    }

    Err("No valid YAML frontmatter found".to_string())
}

pub async fn get_skill_content(
    skill_path: String,
    assistant_id: Option<String>,
    session_id: Option<String>,
    workspace_path: Option<String>,
) -> Result<String, String> {
    let (system_dir, user_dir, assistant_dir, workspace_dir) = resolve_skill_directories(
        assistant_id.as_deref(),
        session_id.as_deref(),
        workspace_path.as_deref().map(Path::new),
    )
    .await?;
    let allowed_roots =
        collect_allowed_skill_roots(system_dir, user_dir, assistant_dir, workspace_dir);
    get_skill_content_from_roots(skill_path, &allowed_roots).await
}

pub async fn get_skill_content_from_roots(
    skill_path: String,
    allowed_roots: &[PathBuf],
) -> Result<String, String> {
    let path = PathBuf::from(&skill_path);

    if path.file_name() != Some(OsStr::new(SKILL_FILE_NAME)) {
        return Err("Skill path must point to a SKILL.md file".to_string());
    }

    let canonical_path = path
        .canonicalize()
        .map_err(|error| format!("Invalid skill path: {}", error))?;

    let mut is_allowed = false;
    for root in allowed_roots {
        if !root.exists() {
            continue;
        }
        let canonical_root = root
            .canonicalize()
            .map_err(|error| format!("Invalid skills directory: {}", error))?;
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
            .map_err(|error| format!("Failed to read skill content: {}", error))
    })
    .await
    .map_err(|error| format!("Task join error: {}", error))?
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if !dst.exists() {
        fs::create_dir_all(dst).map_err(|error| error.to_string())?;
    }

    for entry in fs::read_dir(src).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}
