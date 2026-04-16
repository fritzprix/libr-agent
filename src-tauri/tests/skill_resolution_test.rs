/// Skill Resolution Logic Integration Tests
///
/// Tests additive skill resolution with deterministic precedence:
/// - workspace skills win over assistant/user/system on name collision
/// - assistant skills win over user/system on name collision
/// - user skills win over system on name collision
/// - non-colliding skills from all sources are preserved
/// - No dirs exist → returns empty vec
/// - Results are sorted by name
use std::fs;
use std::path::Path;
use tauri_mcp_agent_lib::services::skill_service::resolve_skills;
use tempfile::TempDir;

/// Helper: create a SKILL.md with valid frontmatter at `dir/subdir/SKILL.md`
fn create_skill(dir: &Path, subdir: &str, name: &str, description: &str) {
    let skill_dir = dir.join(subdir);
    fs::create_dir_all(&skill_dir).unwrap();
    let content = format!(
        "---\nname: {}\ndescription: {}\n---\n# Content for {}\n",
        name, description, name
    );
    fs::write(skill_dir.join("SKILL.md"), content).unwrap();
}

#[tokio::test]
async fn test_resolve_skills_global_only() {
    let system = TempDir::new().unwrap();
    let user = TempDir::new().unwrap();
    create_skill(system.path(), "skill-a", "Skill A", "Description A");

    let result = resolve_skills(system.path().to_owned(), user.path().to_owned(), None, None)
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "Skill A");
    assert_eq!(result[0].source.as_deref(), Some("global"));
    assert_eq!(result[0].origin.as_deref(), Some("system"));
}

#[tokio::test]
async fn test_resolve_skills_assistant_adds_to_global() {
    let system = TempDir::new().unwrap();
    let user = TempDir::new().unwrap();
    let assistant = TempDir::new().unwrap();

    create_skill(system.path(), "global-skill", "Global Skill", "From global");
    create_skill(
        assistant.path(),
        "assistant-skill",
        "Assistant Skill",
        "From assistant",
    );

    let result = resolve_skills(
        system.path().to_owned(),
        user.path().to_owned(),
        Some(assistant.path().to_owned()),
        None,
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].name, "Assistant Skill");
    assert_eq!(result[0].source.as_deref(), Some("assistant"));
    assert_eq!(result[1].name, "Global Skill");
    assert_eq!(result[1].source.as_deref(), Some("global"));
    assert_eq!(result[1].origin.as_deref(), Some("system"));
}

#[tokio::test]
async fn test_resolve_skills_empty_assistant_falls_back_to_global() {
    let system = TempDir::new().unwrap();
    let user = TempDir::new().unwrap();
    let assistant = TempDir::new().unwrap(); // exists but has NO SKILL.md inside

    create_skill(system.path(), "global-skill", "Global Skill", "From global");

    let result = resolve_skills(
        system.path().to_owned(),
        user.path().to_owned(),
        Some(assistant.path().to_owned()),
        None,
    )
    .await
    .unwrap();

    // Empty assistant dir → must fall back to global
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "Global Skill");
    assert_eq!(result[0].source.as_deref(), Some("global"));
    assert_eq!(result[0].origin.as_deref(), Some("system"));
}

#[tokio::test]
async fn test_resolve_skills_nonexistent_assistant_falls_back_to_global() {
    let system = TempDir::new().unwrap();
    let user = TempDir::new().unwrap();
    create_skill(system.path(), "global-skill", "Global Skill", "From global");

    // Assistant dir path that doesn't exist
    let nonexistent_assistant = system.path().join("no_such_assistant_dir");

    let result = resolve_skills(
        system.path().to_owned(),
        user.path().to_owned(),
        Some(nonexistent_assistant),
        None,
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "Global Skill");
}

#[tokio::test]
async fn test_resolve_skills_both_nonexistent_returns_empty() {
    let base = TempDir::new().unwrap();
    let system = base.path().join("no_system");
    let user = base.path().join("no_user");
    let assistant = base.path().join("no_assistant");

    let result = resolve_skills(system, user, Some(assistant), None)
        .await
        .unwrap();

    assert!(result.is_empty());
}

#[tokio::test]
async fn test_resolve_skills_no_assistant_dir_returns_empty_if_no_global() {
    let base = TempDir::new().unwrap();
    let system = base.path().join("nonexistent-system");
    let user = base.path().join("nonexistent-user");

    let result = resolve_skills(system, user, None, None).await.unwrap();

    assert!(result.is_empty());
}

#[tokio::test]
async fn test_resolve_skills_results_sorted_by_name() {
    let system = TempDir::new().unwrap();
    let user = TempDir::new().unwrap();
    // Insert out of order
    create_skill(system.path(), "zzz", "Zzz Skill", "Last alphabetically");
    create_skill(system.path(), "aaa", "Aaa Skill", "First alphabetically");
    create_skill(system.path(), "mmm", "Mmm Skill", "Middle alphabetically");

    let result = resolve_skills(system.path().to_owned(), user.path().to_owned(), None, None)
        .await
        .unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0].name, "Aaa Skill");
    assert_eq!(result[1].name, "Mmm Skill");
    assert_eq!(result[2].name, "Zzz Skill");
}

#[tokio::test]
async fn test_resolve_skills_multiple_assistant_skills_preserve_global() {
    let system = TempDir::new().unwrap();
    let user = TempDir::new().unwrap();
    let assistant = TempDir::new().unwrap();

    // 2 global, 3 assistant
    create_skill(system.path(), "g1", "Global 1", "G1");
    create_skill(system.path(), "g2", "Global 2", "G2");
    create_skill(assistant.path(), "a1", "Assistant 1", "A1");
    create_skill(assistant.path(), "a2", "Assistant 2", "A2");
    create_skill(assistant.path(), "a3", "Assistant 3", "A3");

    let result = resolve_skills(
        system.path().to_owned(),
        user.path().to_owned(),
        Some(assistant.path().to_owned()),
        None,
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 5);
    assert_eq!(
        result
            .iter()
            .filter(|s| s.source.as_deref() == Some("assistant"))
            .count(),
        3
    );
    assert_eq!(
        result
            .iter()
            .filter(|s| s.source.as_deref() == Some("global"))
            .count(),
        2
    );
}

#[tokio::test]
async fn test_resolve_skills_same_name_collision_prefers_assistant() {
    let system = TempDir::new().unwrap();
    let user = TempDir::new().unwrap();
    let assistant = TempDir::new().unwrap();

    // Global "Shared Skill"
    create_skill(
        system.path(),
        "shared-skill",
        "Shared Skill",
        "Global version",
    );
    // Assistant "Shared Skill" — same name, different description
    create_skill(
        assistant.path(),
        "shared-skill",
        "Shared Skill",
        "Assistant version",
    );

    let result = resolve_skills(
        system.path().to_owned(),
        user.path().to_owned(),
        Some(assistant.path().to_owned()),
        None,
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "Shared Skill");
    assert_eq!(result[0].description, "Assistant version");
    assert_eq!(result[0].source.as_deref(), Some("assistant"));
}

#[tokio::test]
async fn test_resolve_skills_same_name_collision_prefers_workspace() {
    let system = TempDir::new().unwrap();
    let user = TempDir::new().unwrap();
    let assistant = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();

    create_skill(
        system.path(),
        "shared-skill",
        "Shared Skill",
        "Global version",
    );
    create_skill(
        assistant.path(),
        "shared-skill",
        "Shared Skill",
        "Assistant version",
    );
    create_skill(
        workspace.path(),
        "shared-skill",
        "Shared Skill",
        "Workspace version",
    );

    let result = resolve_skills(
        system.path().to_owned(),
        user.path().to_owned(),
        Some(assistant.path().to_owned()),
        Some(workspace.path().to_owned()),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].description, "Workspace version");
    assert_eq!(result[0].source.as_deref(), Some("workspace"));
}

#[tokio::test]
async fn test_resolve_skills_same_name_collision_prefers_user_over_system() {
    let system = TempDir::new().unwrap();
    let user = TempDir::new().unwrap();

    create_skill(
        system.path(),
        "shared-skill",
        "Shared Skill",
        "System version",
    );
    create_skill(user.path(), "shared-skill", "Shared Skill", "User version");

    let result = resolve_skills(system.path().to_owned(), user.path().to_owned(), None, None)
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].description, "User version");
    assert_eq!(result[0].source.as_deref(), Some("global"));
    assert_eq!(result[0].origin.as_deref(), Some("user"));
}
