use crate::session::get_session_manager;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::fs;
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

#[tauri::command]
pub async fn get_default_skills_directory() -> Result<String, String> {
    let session_manager = get_session_manager()?;
    let skills_dir = session_manager.get_base_data_dir().join("skills");
    Ok(skills_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn open_skills_directory_in_explorer(directory: Option<String>) -> Result<(), String> {
    let skills_dir = if let Some(dir) = directory {
        PathBuf::from(dir)
    } else {
        let session_manager = get_session_manager()?;
        session_manager.get_base_data_dir().join("skills")
    };

    if !skills_dir.exists() {
        return Err("Skills directory does not exist".to_string());
    }

    crate::utils::fs::open_in_file_manager(&skills_dir)
}

pub async fn get_configured_skills_directory() -> Result<String, String> {
    // Always use default directory (auto-copied from bundled_skills on startup)
    // Assistant-specific skills can override via {data_dir}/assistants/{id}/skills
    get_default_skills_directory().await
}

#[tauri::command]
pub async fn get_aggregated_skills(
    assistant_id: Option<String>,
) -> Result<Vec<SkillMetadata>, String> {
    let global_dir_str = get_configured_skills_directory().await?;
    let global_dir = PathBuf::from(global_dir_str);

    let mut assistant_dir = None;
    let mut _disabled_skills: Option<Vec<String>> = None;

    if let Some(id) = assistant_id {
        let session_manager = get_session_manager()?;
        assistant_dir = Some(
            session_manager
                .get_base_data_dir()
                .join("assistants")
                .join(&id)
                .join("skills"),
        );
    }

    resolve_skills(global_dir, assistant_dir).await
}

#[tauri::command]
pub async fn scan_skills_directory(directory: String) -> Result<Vec<SkillMetadata>, String> {
    scan_skills_internal(&PathBuf::from(directory), None).await
}

/// Returns skills for an assistant. If assistant has skills, returns ONLY those.
/// Otherwise, returns global skills. No merging.
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

async fn scan_skills_internal(
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

fn parse_skill_metadata(path: &Path) -> Result<SkillMetadata, String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_skill_metadata_valid() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("SKILL.md");
        let mut file = fs::File::create(&file_path).unwrap();

        let content = r#"---
name: Test Skill
description: A test skill
---
# Content
"#;
        file.write_all(content.as_bytes()).unwrap();

        let metadata = parse_skill_metadata(&file_path).unwrap();
        assert_eq!(metadata.name, "Test Skill");
        assert_eq!(metadata.description, "A test skill");
        assert_eq!(metadata.path, file_path.to_string_lossy());
    }

    #[test]
    fn test_parse_skill_metadata_missing_frontmatter() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("SKILL.md");
        let mut file = fs::File::create(&file_path).unwrap();

        let content = "# Just markdown content";
        file.write_all(content.as_bytes()).unwrap();

        let result = parse_skill_metadata(&file_path);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "No valid YAML frontmatter found");
    }

    #[test]
    fn test_parse_skill_metadata_invalid_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("SKILL.md");
        let mut file = fs::File::create(&file_path).unwrap();

        let content = r#"---
name: [Invalid YAML
---
"#;
        file.write_all(content.as_bytes()).unwrap();

        let result = parse_skill_metadata(&file_path);
        assert!(result.is_err());
        assert!(result.err().unwrap().contains("YAML parse error"));
    }
}
