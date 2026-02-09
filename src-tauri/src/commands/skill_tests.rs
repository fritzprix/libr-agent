use crate::commands::skill_commands::resolve_skills;
use std::fs;
use std::io::Write;
use tempfile::TempDir;

#[tokio::test]
async fn test_resolve_skills_override() {
    let global_dir = TempDir::new().unwrap();
    let assistant_dir = TempDir::new().unwrap();

    // 1. Create Global Skill: "Skill A"
    create_skill(global_dir.path(), "Skill A", "Global description");

    // 2. Create Global Skill: "Skill B" (Unique to global)
    create_skill(global_dir.path(), "Skill B", "Global description B");

    // 3. Create Assistant Skill: "Skill A" (Override)
    let assistant_skill_path =
        create_skill(assistant_dir.path(), "Skill A", "Assistant description");

    // 4. Create Assistant Skill: "Skill C" (Unique to assistant)
    create_skill(assistant_dir.path(), "Skill C", "Assistant description C");

    // Resolve
    let skills = resolve_skills(
        global_dir.path().to_path_buf(),
        Some(assistant_dir.path().to_path_buf()),
    )
    .await
    .unwrap();

    // Verify
    assert_eq!(skills.len(), 3); // A, B, C

    // Skill A should be from Assistant
    let skill_a = skills.iter().find(|s| s.name == "Skill A").unwrap();
    assert_eq!(skill_a.description, "Assistant description");
    assert_eq!(skill_a.path, assistant_skill_path.to_string_lossy());
    assert_eq!(skill_a.source.as_deref(), Some("assistant"));

    // Skill B should be from Global
    let skill_b = skills.iter().find(|s| s.name == "Skill B").unwrap();
    assert_eq!(skill_b.description, "Global description B");
    assert_eq!(skill_b.source.as_deref(), Some("global"));

    // Skill C should be from Assistant
    let skill_c = skills.iter().find(|s| s.name == "Skill C").unwrap();
    assert_eq!(skill_c.description, "Assistant description C");
    assert_eq!(skill_c.source.as_deref(), Some("assistant"));
}

#[tokio::test]
async fn test_resolve_skills_no_assistant() {
    let global_dir = TempDir::new().unwrap();
    create_skill(global_dir.path(), "Skill A", "Global description");

    let skills = resolve_skills(global_dir.path().to_path_buf(), None)
        .await
        .unwrap();

    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "Skill A");
    assert_eq!(skills[0].source.as_deref(), Some("global"));
}

fn create_skill(dir: &std::path::Path, name: &str, description: &str) -> std::path::PathBuf {
    let skill_name_slug = name.replace(" ", "_").to_lowercase();
    let skill_dir = dir.join(&skill_name_slug);
    fs::create_dir_all(&skill_dir).unwrap();

    let skill_file = skill_dir.join("SKILL.md");
    let mut file = fs::File::create(&skill_file).unwrap();
    writeln!(
        file,
        "---\nname: {}\ndescription: {}\n---\nContent",
        name, description
    )
    .unwrap();

    skill_file
}
