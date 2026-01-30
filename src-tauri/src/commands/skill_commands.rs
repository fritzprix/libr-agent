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

#[derive(Debug, Serialize, Deserialize)]
pub struct SkillsError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for SkillsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap_or_else(|_| self.message.clone()))
    }
}

/// Create a skills directory if it doesn't exist.
/// This is a separate, explicit operation from scanning.
#[tauri::command]
pub async fn create_skills_directory(directory: String) -> Result<(), String> {
    let path = PathBuf::from(&directory);
    
    if path.exists() {
        return Ok(()); // Directory already exists
    }
    
    tokio::task::spawn_blocking(move || {
        fs::create_dir_all(&path)
            .map_err(|e| format!("Failed to create directory: {}", e))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Scan a directory for SKILL.md files and extract their metadata.
/// This function is side-effect free: it returns an error if the directory doesn't exist,
/// rather than creating it. Directory creation should be done explicitly in a separate step.
#[tauri::command]
pub async fn scan_skills_directory(directory: String) -> Result<Vec<SkillMetadata>, String> {
    let root_path = PathBuf::from(&directory);

    // Don't create directories as a side effect of scanning
    if !root_path.exists() {
        let error = SkillsError {
            code: "DIRECTORY_NOT_FOUND".to_string(),
            message: format!("Skills directory does not exist: {}", directory),
        };
        return Err(error.to_string());
    }

    // Offload blocking I/O operations to a blocking thread pool
    // Clone root_path before moving it into the closure
    let root_path_clone = root_path.clone();
    tokio::task::spawn_blocking(move || scan_skills_blocking(&root_path_clone))
        .await
        .map_err(|e| {
            let error = SkillsError {
                code: "TASK_JOIN_ERROR".to_string(),
                message: format!("Task join error: {}", e),
            };
            error.to_string()
        })?
}

/// Blocking implementation of directory scanning.
/// This function performs all filesystem I/O on a blocking thread.
fn scan_skills_blocking(root_path: &Path) -> Result<Vec<SkillMetadata>, String> {
    let mut skills = Vec::new();

    info!("Scanning skills directory: {}", root_path.display());

    // Scan for SKILL.md files
    // - follow_links(false): Don't follow symlinks to avoid cycles and unexpected traversal
    // - Handle errors explicitly instead of silently dropping them
    for entry in WalkDir::new(root_path)
        .follow_links(false)
        .into_iter()
    {
        match entry {
            Ok(entry) => {
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
            Err(e) => {
                // Surface permission/traversal errors to caller for diagnostics
                warn!("Error traversing directory: {}", e);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Test parsing valid frontmatter with proper delimiters
    #[test]
    fn test_parse_valid_frontmatter() {
        let content = r#"---
name: Test Skill
description: A test skill for unit testing
---

# Skill Content

This is the main content of the skill.
"#;

        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(content.as_bytes())
            .expect("Failed to write to temp file");
        temp_file.flush().expect("Failed to flush temp file");

        let result = parse_skill_metadata(temp_file.path());
        assert!(result.is_ok(), "Failed to parse valid frontmatter");

        let metadata = result.unwrap();
        assert_eq!(metadata.name, "Test Skill");
        assert_eq!(metadata.description, "A test skill for unit testing");
    }

    /// Test parsing invalid YAML in frontmatter
    #[test]
    fn test_parse_invalid_yaml() {
        let content = r#"---
name: Test Skill
description: [ invalid yaml
---

# Content
"#;

        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(content.as_bytes())
            .expect("Failed to write to temp file");
        temp_file.flush().expect("Failed to flush temp file");

        let result = parse_skill_metadata(temp_file.path());
        assert!(result.is_err(), "Should fail on invalid YAML");
        assert!(result.unwrap_err().contains("YAML parse error"));
    }

    /// Test file with missing frontmatter delimiters
    #[test]
    fn test_parse_missing_frontmatter() {
        let content = r#"# Skill Without Frontmatter

This file has no YAML frontmatter at all.
"#;

        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(content.as_bytes())
            .expect("Failed to write to temp file");
        temp_file.flush().expect("Failed to flush temp file");

        let result = parse_skill_metadata(temp_file.path());
        assert!(result.is_err(), "Should fail when no frontmatter present");
        assert!(result.unwrap_err().contains("No valid YAML frontmatter"));
    }

    /// Test file with --- appearing later in the markdown body
    #[test]
    fn test_parse_frontmatter_with_delimiter_in_body() {
        let content = r#"---
name: Test Skill
description: A skill with delimiter in body
---

# Skill Content

Here is some markdown content.

---

And here is a horizontal rule that should not confuse the parser.
"#;

        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(content.as_bytes())
            .expect("Failed to write to temp file");
        temp_file.flush().expect("Failed to flush temp file");

        let result = parse_skill_metadata(temp_file.path());
        assert!(result.is_ok(), "Should parse correctly despite --- in body");

        let metadata = result.unwrap();
        assert_eq!(metadata.name, "Test Skill");
        assert_eq!(metadata.description, "A skill with delimiter in body");
    }

    /// Test file with incomplete frontmatter (missing closing delimiter)
    #[test]
    fn test_parse_incomplete_frontmatter() {
        let content = r#"---
name: Test Skill
description: Missing closing delimiter

# Content starts without closing delimiter
"#;

        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(content.as_bytes())
            .expect("Failed to write to temp file");
        temp_file.flush().expect("Failed to flush temp file");

        let result = parse_skill_metadata(temp_file.path());
        assert!(result.is_err(), "Should fail when closing delimiter missing");
    }

    /// Test file with missing required fields in YAML
    #[test]
    fn test_parse_missing_required_fields() {
        let content = r#"---
name: Test Skill
---

# Content
"#;

        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(content.as_bytes())
            .expect("Failed to write to temp file");
        temp_file.flush().expect("Failed to flush temp file");

        let result = parse_skill_metadata(temp_file.path());
        assert!(
            result.is_err(),
            "Should fail when required field 'description' is missing"
        );
        assert!(result.unwrap_err().contains("YAML parse error"));
    }

    /// Test async scan_skills_directory with non-existent directory
    #[tokio::test]
    async fn test_scan_nonexistent_directory() {
        let result = scan_skills_directory("/nonexistent/path/12345".to_string()).await;
        assert!(result.is_err(), "Should return error for non-existent path");
        assert!(result.unwrap_err().contains("does not exist"));
    }
}

