use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[path = "../build_support/bundled_skills.rs"]
mod bundled_skills;

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
fn mirror_bundled_skills_copies_only_valid_skill_directories() {
    let source_root = TempDir::new().unwrap();
    let deployed_root = TempDir::new().unwrap();

    write_skill(
        &source_root.path().join("teamwork"),
        "teamwork",
        "Bundled teamwork",
        "expected bundled body",
    );
    fs::create_dir_all(source_root.path().join("dummy-test")).unwrap();

    bundled_skills::mirror_bundled_skills(source_root.path(), deployed_root.path()).unwrap();

    assert!(deployed_root
        .path()
        .join("teamwork")
        .join("SKILL.md")
        .exists());
    assert!(!deployed_root.path().join("dummy-test").exists());
}

#[test]
fn mirror_bundled_skills_removes_stale_output_for_invalidated_skill_directory() {
    let source_root = TempDir::new().unwrap();
    let deployed_root = TempDir::new().unwrap();

    let source_dummy = source_root.path().join("dummy-test");
    write_skill(
        &source_dummy,
        "dummy-test",
        "Bundled dummy",
        "present on first sync",
    );

    bundled_skills::mirror_bundled_skills(source_root.path(), deployed_root.path()).unwrap();
    assert!(deployed_root
        .path()
        .join("dummy-test")
        .join("SKILL.md")
        .exists());

    fs::remove_file(source_dummy.join("SKILL.md")).unwrap();

    bundled_skills::mirror_bundled_skills(source_root.path(), deployed_root.path()).unwrap();

    assert!(!deployed_root.path().join("dummy-test").exists());
}
