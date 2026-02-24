use std::fs;
use std::io::Write;
use tauri_mcp_agent_lib::services::skill_service::parse_skill_metadata;
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
