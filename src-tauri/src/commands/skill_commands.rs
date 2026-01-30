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
}

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: String,
}

#[tauri::command]
pub async fn scan_skills_directory(directory: String) -> Result<Vec<SkillMetadata>, String> {
    let root_path = PathBuf::from(&directory);

    if !root_path.exists() {
        info!("Skills directory does not exist, creating: {}", directory);
        fs::create_dir_all(&root_path)
            .map_err(|e| format!("Failed to create skills directory: {}", e))?;
    }

    let mut skills = Vec::new();

    info!("Scanning skills directory: {}", directory);

    // We scan specifically for SKILL.md files
    for entry in WalkDir::new(&root_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name() == "SKILL.md" {
            let path = entry.path();
            match parse_skill_metadata(path) {
                Ok(metadata) => {
                    skills.push(metadata);
                }
                Err(e) => {
                    warn!("Failed to parse skill at {:?}: {}", path, e);
                }
            }
        }
    }

    Ok(skills)
}

fn parse_skill_metadata(path: &Path) -> Result<SkillMetadata, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;

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
            });
        }
    }

    Err("No valid YAML frontmatter found".to_string())
}
