use tauri_mcp_agent_lib::services::skill_service::{
    build_skill_import_conflicts, SkillImportCandidate, SkillMetadata,
};

fn metadata(name: &str, origin: &str, path: &str) -> SkillMetadata {
    SkillMetadata {
        name: name.to_string(),
        description: format!("{} description", name),
        path: path.to_string(),
        source: Some("global".to_string()),
        origin: Some(origin.to_string()),
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
