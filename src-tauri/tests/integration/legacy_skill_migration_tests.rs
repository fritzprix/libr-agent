use std::fs;
use std::path::Path;
use tauri_mcp_agent_lib::lifecycle::app_setup::{
    classify_legacy_skill_for_managed_storage, hash_skill_directory,
    remove_legacy_skills_dir_if_empty, replace_skill_directory_atomically,
    sync_managed_system_skills_snapshot, write_manifest_atomically, BundledSkillsManifest,
    LegacySkillMigrationAction,
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
fn sync_managed_system_skills_restores_tampered_skill_contents_when_manifest_is_missing() {
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
    fs::remove_file(
        system_root
            .path()
            .join(MANAGED_SYSTEM_SKILLS_MANIFEST_FILE_NAME),
    )
    .unwrap();

    sync_managed_system_skills_snapshot(bundled_root.path(), system_root.path()).unwrap();

    let teamwork_skill =
        fs::read_to_string(system_root.path().join("teamwork").join("SKILL.md")).unwrap();
    assert!(teamwork_skill.contains("Bundled teamwork"));
    assert!(teamwork_skill.contains("expected bundled body"));
}

#[test]
fn sync_managed_system_skills_rebuilds_invalid_manifest_before_comparing() {
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
        system_root
            .path()
            .join(MANAGED_SYSTEM_SKILLS_MANIFEST_FILE_NAME),
        b"{not-json",
    )
    .unwrap();
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

#[test]
fn test_hash_skill_directory_deterministic() {
    let root = TempDir::new().unwrap();
    let skill_dir = root.path().join("my-skill");
    write_skill(&skill_dir, "my-skill", "Desc", "body content");

    // Hash first time
    let hash1 = hash_skill_directory(&skill_dir).unwrap();

    // Hashing again should yield the exact same value
    let hash2 = hash_skill_directory(&skill_dir).unwrap();
    assert_eq!(hash1, hash2);

    // Adding `.bundled_skill` marker file should be ignored in the directory hash
    fs::write(skill_dir.join(".bundled_skill"), b"bundled").unwrap();
    let hash3 = hash_skill_directory(&skill_dir).unwrap();
    assert_eq!(hash1, hash3);

    // Modifying the body content should change the hash
    write_skill(&skill_dir, "my-skill", "Desc", "modified content");
    let hash4 = hash_skill_directory(&skill_dir).unwrap();
    assert_ne!(hash1, hash4);
}

#[test]
fn test_write_manifest_atomically_success_and_recovery() {
    use std::collections::BTreeMap;

    let root = TempDir::new().unwrap();
    let manifest_path = root.path().join(".bundled_skills_manifest.json");

    let mut skills = BTreeMap::new();
    skills.insert("skill1".to_string(), "hash1".to_string());
    let manifest = BundledSkillsManifest {
        schema_version: 1,
        skills,
    };

    // 1. Success path
    write_manifest_atomically(&manifest_path, &manifest).unwrap();
    assert!(manifest_path.exists());

    let content = fs::read_to_string(&manifest_path).unwrap();
    assert!(content.contains("skill1"));
    assert!(content.contains("hash1"));

    // Check no temp or backup files are left behind on success
    let temp_path = manifest_path.with_extension("json.tmp");
    let backup_path = manifest_path.with_extension("json.bak");
    assert!(!temp_path.exists());
    assert!(!backup_path.exists());

    // 2. Modifying and rewriting manifest
    let mut skills_v2 = BTreeMap::new();
    skills_v2.insert("skill1".to_string(), "hash_updated".to_string());
    let manifest_v2 = BundledSkillsManifest {
        schema_version: 1,
        skills: skills_v2,
    };

    write_manifest_atomically(&manifest_path, &manifest_v2).unwrap();
    let content_v2 = fs::read_to_string(&manifest_path).unwrap();
    assert!(content_v2.contains("hash_updated"));
    assert!(!temp_path.exists());
    assert!(!backup_path.exists());
}

#[test]
fn test_replace_skill_directory_atomically_cleans_temp_and_backup() {
    let root = TempDir::new().unwrap();
    let source_dir = root.path().join("source-skill");
    let target_dir = root.path().join("target-skill");

    write_skill(&source_dir, "my-skill", "Source Desc", "source content");

    // Pre-create temp and backup directories to simulate a crashed previous run
    let parent = target_dir.parent().unwrap();
    let temp_dir = parent.join(".sync-tmp-target-skill");
    let backup_dir = parent.join(".sync-backup-target-skill");
    fs::create_dir_all(&temp_dir).unwrap();
    fs::create_dir_all(&backup_dir).unwrap();

    assert!(temp_dir.exists());
    assert!(backup_dir.exists());

    // Replace directory atomically
    replace_skill_directory_atomically(&source_dir, &target_dir).unwrap();

    // The target directory must now exist, with the correct contents and bundled marker
    assert!(target_dir.exists());
    assert!(target_dir.join("SKILL.md").exists());
    assert!(target_dir.join(".bundled_skill").exists());

    let content = fs::read_to_string(target_dir.join("SKILL.md")).unwrap();
    assert!(content.contains("source content"));

    // Pre-existing temp and backup directories must be cleaned up on sync
    assert!(!temp_dir.exists());
    assert!(!backup_dir.exists());
}
