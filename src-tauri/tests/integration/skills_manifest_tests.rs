use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use tauri_mcp_agent_lib::lifecycle::app_setup::{
    hash_skill_directory, load_persisted_bundled_skills_manifest,
    replace_skill_directory_atomically, write_manifest_atomically, BundledSkillsManifest,
};
use tempfile::TempDir;

const EMPTY_DIR_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

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
fn hash_skill_directory_empty_directory() {
    let root = TempDir::new().unwrap();
    let skill_dir = root.path().join("empty-skill");
    fs::create_dir_all(&skill_dir).unwrap();

    let hash = hash_skill_directory(&skill_dir).unwrap();
    assert_eq!(hash, EMPTY_DIR_SHA256);
}

#[test]
fn load_persisted_bundled_skills_manifest_returns_none_for_missing_file() {
    let root = TempDir::new().unwrap();
    let manifest_path = root.path().join("missing-manifest.json");

    assert_eq!(
        load_persisted_bundled_skills_manifest(&manifest_path).unwrap(),
        None
    );
}

#[test]
fn load_persisted_bundled_skills_manifest_returns_none_for_corrupt_json() {
    let root = TempDir::new().unwrap();
    let manifest_path = root.path().join(".bundled_skills_manifest.json");
    fs::write(&manifest_path, b"not-json").unwrap();

    assert_eq!(
        load_persisted_bundled_skills_manifest(&manifest_path).unwrap(),
        None
    );
}

#[test]
fn load_persisted_bundled_skills_manifest_returns_none_for_unsupported_schema() {
    let root = TempDir::new().unwrap();
    let manifest_path = root.path().join(".bundled_skills_manifest.json");
    fs::write(&manifest_path, br#"{"schemaVersion":999,"skills":{}}"#).unwrap();

    assert_eq!(
        load_persisted_bundled_skills_manifest(&manifest_path).unwrap(),
        None
    );
}

#[test]
fn load_persisted_bundled_skills_manifest_reads_valid_manifest() {
    let root = TempDir::new().unwrap();
    let manifest_path = root.path().join(".bundled_skills_manifest.json");
    fs::write(
        &manifest_path,
        br#"{"schemaVersion":1,"skills":{"alpha":"abc123"}}"#,
    )
    .unwrap();

    let manifest = load_persisted_bundled_skills_manifest(&manifest_path)
        .unwrap()
        .expect("valid manifest should load");
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.skills.get("alpha"), Some(&"abc123".to_string()));
}

#[test]
fn write_manifest_atomically_leaves_no_staging_artifacts_on_success() {
    let root = TempDir::new().unwrap();
    let manifest_path = root.path().join(".bundled_skills_manifest.json");
    let temp_path = manifest_path.with_extension("json.tmp");
    let backup_path = manifest_path.with_extension("json.bak");

    let mut skills = BTreeMap::new();
    skills.insert("skill1".to_string(), "hash1".to_string());
    let manifest = BundledSkillsManifest {
        schema_version: 1,
        skills,
    };

    write_manifest_atomically(&manifest_path, &manifest).unwrap();

    assert!(manifest_path.exists());
    assert!(!temp_path.exists());
    assert!(!backup_path.exists());
}

#[test]
fn write_manifest_atomically_clears_stale_temp_directory() {
    let root = TempDir::new().unwrap();
    let manifest_path = root.path().join(".bundled_skills_manifest.json");
    let temp_path = manifest_path.with_extension("json.tmp");
    fs::create_dir_all(&temp_path).unwrap();

    let manifest = BundledSkillsManifest {
        schema_version: 1,
        skills: BTreeMap::new(),
    };

    write_manifest_atomically(&manifest_path, &manifest).unwrap();

    assert!(manifest_path.exists());
    assert!(!temp_path.exists());
}

#[test]
fn replace_skill_directory_atomically_leaves_no_staging_artifacts_on_success() {
    let root = TempDir::new().unwrap();
    let source_dir = root.path().join("source-skill");
    let target_dir = root.path().join("target-skill");
    let parent = target_dir.parent().unwrap();
    let temp_dir = parent.join(".sync-tmp-target-skill");
    let backup_dir = parent.join(".sync-backup-target-skill");

    write_skill(&source_dir, "target-skill", "Source", "source body");
    fs::create_dir_all(&temp_dir).unwrap();
    fs::create_dir_all(&backup_dir).unwrap();

    replace_skill_directory_atomically(&source_dir, &target_dir).unwrap();

    assert!(target_dir.exists());
    assert!(target_dir.join("SKILL.md").exists());
    assert!(target_dir.join(".bundled_skill").exists());
    assert!(!temp_dir.exists());
    assert!(!backup_dir.exists());
}
