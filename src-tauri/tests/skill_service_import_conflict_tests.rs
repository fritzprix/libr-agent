use std::fs;
use std::path::{Path, PathBuf};
use tauri_mcp_agent_lib::commands::skill_management::import_user_skills;
use tauri_mcp_agent_lib::services::skill_service::{
    build_skill_import_conflicts, get_system_skills_directory, get_user_skills_directory,
    skill_storage_directory_name, SkillImportCandidate, SkillMetadata, SKILL_FILE_NAME,
};
use tempfile::TempDir;
use uuid::Uuid;

fn metadata(name: &str, origin: &str, path: &str) -> SkillMetadata {
    SkillMetadata {
        name: name.to_string(),
        description: format!("{} description", name),
        path: path.to_string(),
        source: Some("global".to_string()),
        origin: Some(origin.to_string()),
    }
}

fn unique_skill_name(prefix: &str) -> String {
    format!("{}-{}", prefix, Uuid::new_v4())
}

fn create_skill_dir(root: &Path, skill_name: &str, description: &str) -> PathBuf {
    let storage_name = skill_storage_directory_name(skill_name).expect("storage name");
    let skill_dir = root.join(storage_name);
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join(SKILL_FILE_NAME),
        format!(
            "---\nname: {}\ndescription: {}\n---\n# {}\n",
            skill_name, description, skill_name
        ),
    )
    .expect("write skill file");
    skill_dir
}

fn create_import_source(skills: &[(&str, &str)]) -> TempDir {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    for (name, description) in skills {
        let skill_dir = temp_dir.path().join(name);
        fs::create_dir_all(&skill_dir).expect("create import skill dir");
        fs::write(
            skill_dir.join(SKILL_FILE_NAME),
            format!(
                "---\nname: {}\ndescription: {}\n---\n# {}\n",
                name, description, name
            ),
        )
        .expect("write import skill");
    }
    temp_dir
}

fn remove_if_exists(path: &Path) {
    if path.exists() {
        fs::remove_dir_all(path).expect("remove test skill dir");
    }
}

#[test]
fn import_conflicts_prefer_system_skills_over_user_skills() {
    let discovered = vec![SkillImportCandidate {
        name: "teamwork".to_string(),
        description: "Incoming teamwork".to_string(),
    }];
    let system_skills = vec![metadata(
        "teamwork",
        "system",
        "/app/bundled/teamwork/SKILL.md",
    )];
    let user_skills = vec![metadata("teamwork", "user", "/data/user/teamwork/SKILL.md")];

    let conflicts = build_skill_import_conflicts(&discovered, &system_skills, &user_skills);

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].name, "teamwork");
    assert_eq!(conflicts[0].existing_origin, "system");
    assert_eq!(conflicts[0].existing_path, "/app/bundled/teamwork/SKILL.md");
}

#[test]
fn import_conflicts_use_user_origin_when_no_system_skill_exists() {
    let discovered = vec![SkillImportCandidate {
        name: "custom-helper".to_string(),
        description: "Incoming helper".to_string(),
    }];
    let system_skills = vec![metadata(
        "teamwork",
        "system",
        "/app/bundled/teamwork/SKILL.md",
    )];
    let user_skills = vec![metadata(
        "custom-helper",
        "user",
        "/data/user/custom-helper/SKILL.md",
    )];

    let conflicts = build_skill_import_conflicts(&discovered, &system_skills, &user_skills);

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].name, "custom-helper");
    assert_eq!(conflicts[0].existing_origin, "user");
    assert_eq!(
        conflicts[0].existing_path,
        "/data/user/custom-helper/SKILL.md"
    );
}

#[tokio::test]
async fn import_user_skills_skips_system_conflicts_even_with_overwrite() {
    let system_dir = get_system_skills_directory().expect("system dir");
    let user_dir = get_user_skills_directory().expect("user dir");
    let system_skill_name = unique_skill_name("system-collision");
    let imported_skill_name = unique_skill_name("fresh-import");

    let system_skill_dir = create_skill_dir(&system_dir, &system_skill_name, "bundled");
    let import_source = create_import_source(&[
        (&system_skill_name, "incoming-system-collision"),
        (&imported_skill_name, "incoming-fresh"),
    ]);

    let imported_skill_dir =
        user_dir.join(skill_storage_directory_name(&imported_skill_name).expect("storage name"));

    let result = import_user_skills(
        import_source.path().to_string_lossy().to_string(),
        true,
        None,
    )
    .await
    .expect("import result");

    assert!(result.imported_names.contains(&imported_skill_name));
    assert!(result.skipped_names.contains(&system_skill_name));
    assert!(!result.overwritten_names.contains(&system_skill_name));

    remove_if_exists(&system_skill_dir);
    remove_if_exists(&imported_skill_dir);
}

#[tokio::test]
async fn import_user_skills_skips_excluded_user_conflicts_without_overwrite() {
    let user_dir = get_user_skills_directory().expect("user dir");
    let existing_skill_name = unique_skill_name("excluded-user-collision");
    let imported_skill_name = unique_skill_name("excluded-user-fresh");

    let existing_skill_dir = create_skill_dir(&user_dir, &existing_skill_name, "existing user");
    let import_source = create_import_source(&[
        (&existing_skill_name, "incoming user collision"),
        (&imported_skill_name, "incoming fresh"),
    ]);
    let imported_skill_dir =
        user_dir.join(skill_storage_directory_name(&imported_skill_name).expect("storage name"));

    let result = import_user_skills(
        import_source.path().to_string_lossy().to_string(),
        false,
        Some(vec![existing_skill_name.clone()]),
    )
    .await
    .expect("import result");

    assert!(result.imported_names.contains(&imported_skill_name));
    assert!(result.skipped_names.contains(&existing_skill_name));
    assert!(result.overwritten_names.is_empty());

    remove_if_exists(&existing_skill_dir);
    remove_if_exists(&imported_skill_dir);
}

#[tokio::test]
async fn import_user_skills_requires_overwrite_for_included_user_conflicts() {
    let user_dir = get_user_skills_directory().expect("user dir");
    let existing_skill_name = unique_skill_name("overwrite-user-collision");

    let existing_skill_dir = create_skill_dir(&user_dir, &existing_skill_name, "existing user");
    let import_source = create_import_source(&[(&existing_skill_name, "incoming overwrite")]);

    let error = import_user_skills(
        import_source.path().to_string_lossy().to_string(),
        false,
        None,
    )
    .await
    .expect_err("conflict should require overwrite");
    assert!(error.contains("excluded_skill_names"));

    let overwrite_result = import_user_skills(
        import_source.path().to_string_lossy().to_string(),
        true,
        None,
    )
    .await
    .expect("overwrite result");
    assert!(overwrite_result
        .overwritten_names
        .contains(&existing_skill_name));
    assert!(overwrite_result.skipped_names.is_empty());

    remove_if_exists(&existing_skill_dir);
}
