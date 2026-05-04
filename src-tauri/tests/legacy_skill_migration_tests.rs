use std::fs;
use std::path::Path;
use tauri_mcp_agent_lib::lifecycle::app_setup::{
    classify_legacy_skill_for_managed_storage, remove_legacy_skills_dir_if_empty,
    sync_managed_system_skills_snapshot, LegacySkillMigrationAction,
};
use tauri_mcp_agent_lib::services::skill_service::MANAGED_SYSTEM_SKILLS_MANIFEST_FILE_NAME;
use tempfile::TempDir;

fn write_skill(dir: &Path, name: &str, description: &str, body: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!(
            "---\nname: {}\ndescription: {}\n---\n{}\n",
            name, description, body
        ),
    )
    .unwrap();
}

#[test]
fn legacy_skill_matching_current_bundle_is_deleted() {
    let legacy_root = TempDir::new().unwrap();

    let legacy_skill_dir = legacy_root.path().join("teamwork");

    write_skill(
        &legacy_skill_dir,
        "teamwork",
        "Shared description",
        "same body",
    );
    fs::write(legacy_skill_dir.join(".bundled_skill"), "").unwrap();

    let action = classify_legacy_skill_for_managed_storage(&legacy_skill_dir).unwrap();

    assert_eq!(action, LegacySkillMigrationAction::DeleteLegacyCopy);
}

#[test]
fn unmarked_legacy_skill_is_migrated_to_user() {
    let legacy_root = TempDir::new().unwrap();

    let legacy_skill_dir = legacy_root.path().join("custom-skill");
    write_skill(&legacy_skill_dir, "custom-skill", "Custom", "user content");

    let action = classify_legacy_skill_for_managed_storage(&legacy_skill_dir).unwrap();

    assert_eq!(action, LegacySkillMigrationAction::MigrateToUser);
}

#[test]
fn legacy_bundled_skill_not_in_current_bundle_is_deleted() {
    let legacy_root = TempDir::new().unwrap();

    let legacy_skill_dir = legacy_root.path().join("custom-skill");

    write_skill(
        &legacy_skill_dir,
        "custom-skill",
        "Legacy bundled description",
        "legacy bundled content",
    );
    fs::write(legacy_skill_dir.join(".bundled_skill"), "").unwrap();

    let action = classify_legacy_skill_for_managed_storage(&legacy_skill_dir).unwrap();

    assert_eq!(action, LegacySkillMigrationAction::DeleteLegacyCopy);
}

#[test]
fn modified_legacy_bundled_skill_is_deleted_instead_of_migrated() {
    let legacy_root = TempDir::new().unwrap();

    let legacy_skill_dir = legacy_root.path().join("delegate");
    write_skill(
        &legacy_skill_dir,
        "delegate",
        "Modified description",
        "legacy user override",
    );
    fs::write(legacy_skill_dir.join(".bundled_skill"), "").unwrap();

    let action = classify_legacy_skill_for_managed_storage(&legacy_skill_dir).unwrap();

    assert_eq!(action, LegacySkillMigrationAction::DeleteLegacyCopy);
}

#[test]
fn sync_managed_system_skills_removes_extras_and_restores_missing_bundled_skills() {
    let bundled_root = TempDir::new().unwrap();
    let system_root = TempDir::new().unwrap();

    let bundled_teamwork = bundled_root.path().join("teamwork");
    let bundled_delegate = bundled_root.path().join("delegate");
    write_skill(
        &bundled_teamwork,
        "teamwork",
        "Bundled teamwork",
        "team body",
    );
    write_skill(
        &bundled_delegate,
        "delegate",
        "Bundled delegate",
        "delegate body",
    );

    let system_teamwork = system_root.path().join("teamwork");
    let system_extra = system_root.path().join("custom-extra");
    write_skill(
        &system_teamwork,
        "teamwork",
        "Old modified",
        "outdated body",
    );
    write_skill(&system_extra, "custom-extra", "Extra", "extra body");

    sync_managed_system_skills_snapshot(bundled_root.path(), system_root.path()).unwrap();

    assert!(system_root.path().join("teamwork").exists());
    assert!(system_root.path().join("delegate").exists());
    assert!(!system_root.path().join("custom-extra").exists());

    let teamwork_skill =
        fs::read_to_string(system_root.path().join("teamwork").join("SKILL.md")).unwrap();
    assert!(teamwork_skill.contains("Bundled teamwork"));
    assert!(system_root
        .path()
        .join(MANAGED_SYSTEM_SKILLS_MANIFEST_FILE_NAME)
        .exists());
}

#[test]
fn sync_managed_system_skills_restores_tampered_skill_contents() {
    let bundled_root = TempDir::new().unwrap();
    let system_root = TempDir::new().unwrap();

    let bundled_teamwork = bundled_root.path().join("teamwork");
    write_skill(
        &bundled_teamwork,
        "teamwork",
        "Bundled teamwork",
        "expected bundled body",
    );

    sync_managed_system_skills_snapshot(bundled_root.path(), system_root.path()).unwrap();

    fs::write(
        system_root.path().join("teamwork").join("SKILL.md"),
        "---\nname: teamwork\ndescription: Tampered\n---\nlocal drift\n",
    )
    .unwrap();

    sync_managed_system_skills_snapshot(bundled_root.path(), system_root.path()).unwrap();

    let teamwork_skill =
        fs::read_to_string(system_root.path().join("teamwork").join("SKILL.md")).unwrap();
    assert!(teamwork_skill.contains("Bundled teamwork"));
    assert!(teamwork_skill.contains("expected bundled body"));
}

#[test]
fn sync_managed_system_skills_ignores_empty_source_directories() {
    let bundled_root = TempDir::new().unwrap();
    let system_root = TempDir::new().unwrap();

    let bundled_teamwork = bundled_root.path().join("teamwork");
    write_skill(
        &bundled_teamwork,
        "teamwork",
        "Bundled teamwork",
        "expected bundled body",
    );
    fs::create_dir_all(bundled_root.path().join("dummy-test")).unwrap();

    let stale_dummy = system_root.path().join("dummy-test");
    write_skill(&stale_dummy, "dummy-test", "Old dummy", "stale content");

    sync_managed_system_skills_snapshot(bundled_root.path(), system_root.path()).unwrap();

    assert!(system_root.path().join("teamwork").exists());
    assert!(!system_root.path().join("dummy-test").exists());
}

#[test]
fn remove_legacy_skills_dir_if_empty_removes_only_empty_roots() {
    let legacy_root = TempDir::new().unwrap();
    let empty_legacy_dir = legacy_root.path().join("skills");
    fs::create_dir_all(&empty_legacy_dir).unwrap();

    let removed = remove_legacy_skills_dir_if_empty(&empty_legacy_dir).unwrap();
    assert!(removed);
    assert!(!empty_legacy_dir.exists());

    fs::create_dir_all(&empty_legacy_dir).unwrap();
    write_skill(
        &empty_legacy_dir.join("custom-skill"),
        "custom-skill",
        "Custom",
        "user content",
    );

    let removed_non_empty = remove_legacy_skills_dir_if_empty(&empty_legacy_dir).unwrap();
    assert!(!removed_non_empty);
    assert!(empty_legacy_dir.exists());
}
