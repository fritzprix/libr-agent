use std::fs;
use std::path::Path;
use tauri_mcp_agent_lib::lifecycle::app_setup::{
    classify_legacy_skill_for_managed_storage, sync_legacy_global_skills_to_bundled_snapshot,
    LegacySkillMigrationAction,
};
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
fn sync_legacy_global_skills_removes_extras_and_restores_missing_bundled_skills() {
    let bundled_root = TempDir::new().unwrap();
    let legacy_root = TempDir::new().unwrap();

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

    let legacy_teamwork = legacy_root.path().join("teamwork");
    let legacy_extra = legacy_root.path().join("custom-extra");
    write_skill(
        &legacy_teamwork,
        "teamwork",
        "Old modified",
        "outdated body",
    );
    write_skill(&legacy_extra, "custom-extra", "Extra", "extra body");
    fs::write(legacy_teamwork.join(".bundled_skill"), "").unwrap();
    fs::write(legacy_extra.join(".bundled_skill"), "").unwrap();

    sync_legacy_global_skills_to_bundled_snapshot(bundled_root.path(), legacy_root.path()).unwrap();

    assert!(legacy_root.path().join("teamwork").exists());
    assert!(legacy_root.path().join("delegate").exists());
    assert!(!legacy_root.path().join("custom-extra").exists());

    let teamwork_skill =
        fs::read_to_string(legacy_root.path().join("teamwork").join("SKILL.md")).unwrap();
    assert!(teamwork_skill.contains("Bundled teamwork"));
    assert!(legacy_root
        .path()
        .join("teamwork")
        .join(".bundled_skill")
        .exists());
    assert!(legacy_root
        .path()
        .join("delegate")
        .join(".bundled_skill")
        .exists());
}
