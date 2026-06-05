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

#[tokio::test]
async fn test_resolve_skills_agent_auto_discover() {
    let system = TempDir::new().unwrap();
    let user = TempDir::new().unwrap();
    let project_root = TempDir::new().unwrap();

    // workspace_dir = project_root/.libragent/skills
    let workspace_dir = project_root.path().join(".libragent").join("skills");
    fs::create_dir_all(&workspace_dir).unwrap();

    // Create an agent skill in project_root/.agents/skills/my-agent-skill
    let agent_skills_dir = project_root.path().join(".agents").join("skills");
    create_skill(
        &agent_skills_dir,
        "my-agent-skill",
        "Agent Skill",
        "Description Agent",
    );

    let result = resolve_skills(
        system.path().to_owned(),
        user.path().to_owned(),
        None,
        Some(workspace_dir),
    )
    .await
    .unwrap();

    // The agent skill should be auto-discovered
    let agent_skill = result.iter().find(|s| s.name == "Agent Skill");
    assert!(agent_skill.is_some());
    let s = agent_skill.unwrap();
    assert_eq!(s.description, "Description Agent");
    assert_eq!(s.source.as_deref(), Some("agent_import"));
    assert_eq!(s.origin.as_deref(), Some("agent"));
}

#[tokio::test]
async fn test_resolve_skills_same_name_collision_agent_precedence() {
    // Case A: workspace vs agent vs user vs system -> workspace wins
    {
        let system = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let project_root = TempDir::new().unwrap();
        let workspace_dir = project_root.path().join(".libragent").join("skills");
        fs::create_dir_all(&workspace_dir).unwrap();

        create_skill(system.path(), "shared", "Shared", "System version");
        create_skill(user.path(), "shared", "Shared", "User version");
        let agent_skills_dir = project_root.path().join(".agents").join("skills");
        create_skill(&agent_skills_dir, "shared", "Shared", "Agent version");
        create_skill(&workspace_dir, "shared", "Shared", "Workspace version");

        let result = resolve_skills(
            system.path().to_owned(),
            user.path().to_owned(),
            None,
            Some(workspace_dir),
        )
        .await
        .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].description, "Workspace version");
    }

    // Case B: agent vs user vs system (without workspace skill) -> agent wins
    {
        let system = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let project_root = TempDir::new().unwrap();
        let workspace_dir = project_root.path().join(".libragent").join("skills");
        fs::create_dir_all(&workspace_dir).unwrap();

        create_skill(system.path(), "shared", "Shared", "System version");
        create_skill(user.path(), "shared", "Shared", "User version");
        let agent_skills_dir = project_root.path().join(".agents").join("skills");
        create_skill(&agent_skills_dir, "shared", "Shared", "Agent version");

        let result = resolve_skills(
            system.path().to_owned(),
            user.path().to_owned(),
            None,
            Some(workspace_dir),
        )
        .await
        .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].description, "Agent version");
    }

    // Case C: user vs system (without agent skill) -> user wins
    {
        let system = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let project_root = TempDir::new().unwrap();
        let workspace_dir = project_root.path().join(".libragent").join("skills");
        fs::create_dir_all(&workspace_dir).unwrap();

        create_skill(system.path(), "shared", "Shared", "System version");
        create_skill(user.path(), "shared", "Shared", "User version");

        let result = resolve_skills(
            system.path().to_owned(),
            user.path().to_owned(),
            None,
            Some(workspace_dir),
        )
        .await
        .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].description, "User version");
    }
}

#[tokio::test]
async fn test_resolve_skills_agent_auto_discover_all_patterns() {
    let patterns = [
        ".agents/skills",
        ".gemini/skills",
        ".copilot/skills",
        ".cursor/skills",
        ".windsurf/skills",
        ".claude/skills",
        ".cline/skills",
        ".continue/skills",
    ];
    for pattern in &patterns {
        let system = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let project_root = TempDir::new().unwrap();

        // workspace_dir = project_root/.libragent/skills
        let workspace_dir = project_root.path().join(".libragent").join("skills");
        fs::create_dir_all(&workspace_dir).unwrap();

        // Create agent skill in project_root/<pattern>/test-skill
        let agent_skills_dir = project_root.path().join(pattern);
        create_skill(
            &agent_skills_dir,
            "test-skill",
            "Test Skill",
            &format!("From {}", pattern),
        );

        let result = resolve_skills(
            system.path().to_owned(),
            user.path().to_owned(),
            None,
            Some(workspace_dir),
        )
        .await
        .unwrap();

        let skill = result.iter().find(|s| s.name == "Test Skill");
        assert!(
            skill.is_some(),
            "Failed to discover skill under {}",
            pattern
        );
        assert_eq!(skill.unwrap().description, format!("From {}", pattern));
    }
}

#[tokio::test]
async fn test_collect_allowed_skill_roots_includes_agent_dirs() {
    let system = TempDir::new().unwrap();
    let user = TempDir::new().unwrap();
    let project_root = TempDir::new().unwrap();

    let workspace_dir = project_root.path().join(".libragent").join("skills");
    fs::create_dir_all(&workspace_dir).unwrap();

    let agent_dir = project_root.path().join(".agents").join("skills");
    fs::create_dir_all(&agent_dir).unwrap();

    use tauri_mcp_agent_lib::services::skill_service::collect_allowed_skill_roots;
    let roots = collect_allowed_skill_roots(
        system.path().to_owned(),
        user.path().to_owned(),
        None,
        Some(workspace_dir),
    );

    assert!(roots.iter().any(|r| r == &agent_dir));
}

#[tokio::test]
async fn test_resolve_skills_workspace_root_fallback() {
    let system = TempDir::new().unwrap();
    let user = TempDir::new().unwrap();
    let project_root = TempDir::new().unwrap();

    // Create a deeply nested directory with NO .git or .libragent folder anywhere in parent tree
    let nested_dir = project_root
        .path()
        .join("dir1")
        .join("dir2")
        .join("dir3")
        .join("dir4")
        .join("dir5")
        .join("dir6")
        .join("dir7")
        .join("dir8")
        .join("dir9")
        .join("dir10");
    fs::create_dir_all(&nested_dir).unwrap();

    // Invoking resolve_skills on this directory should not crash or loop infinitely,
    // and should gracefully return Ok (likely empty because no agent skills exist there)
    let result = resolve_skills(
        system.path().to_owned(),
        user.path().to_owned(),
        None,
        Some(nested_dir),
    )
    .await;

    assert!(result.is_ok());
}
