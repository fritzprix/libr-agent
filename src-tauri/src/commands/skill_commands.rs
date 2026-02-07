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

#[tauri::command]
pub async fn scan_skills_directory(directory: String) -> Result<Vec<SkillMetadata>, String> {
    let root_path = PathBuf::from(&directory);

    if !root_path.exists() {
        // Critique #3: Side-effect on scan (mkdir) - Removed
        // Also critique #5: Return empty list or error instead of creating
        info!("Skills directory does not exist: {}", directory);
        return Ok(Vec::new());
    }

    info!("Scanning skills directory: {}", directory);

    // Critique #2: Blocking I/O in async. Offload to spawn_blocking.
    tokio::task::spawn_blocking(move || {
        let mut skills = Vec::new();

        // Critique #1: Unintended filesystem access (symlinks). Disable follow_links.
        for entry in WalkDir::new(&root_path)
            .follow_links(false) // Changed from true to false
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
