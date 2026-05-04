use std::fs;
use std::path::{Path, PathBuf};
use tauri_mcp_agent_lib::services::skill_service::{
    get_managed_skills_overview, get_user_skills_directory, invalidate_skill_scan_cache,
    skill_storage_directory_name, SKILL_FILE_NAME,
};
use uuid::Uuid;

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

fn remove_if_exists(path: &Path) {
    if path.exists() {
        fs::remove_dir_all(path).expect("remove test skill dir");
    }
}

#[tokio::test]
async fn managed_skills_overview_uses_cache_until_invalidated() {
    let user_dir = get_user_skills_directory().expect("user dir");
    let skill_name = unique_skill_name("cache-regression");
    let skill_dir = create_skill_dir(&user_dir, &skill_name, "cached skill");

    invalidate_skill_scan_cache();

    let initial = get_managed_skills_overview()
        .await
        .expect("initial overview");
    assert!(
        initial
            .user_skills
            .iter()
            .any(|skill| skill.name == skill_name),
        "initial scan should include the created skill"
    );

    remove_if_exists(&skill_dir);

    let cached = get_managed_skills_overview()
        .await
        .expect("cached overview");
    assert!(
        cached
            .user_skills
            .iter()
            .any(|skill| skill.name == skill_name),
        "cached overview should still expose the removed skill before invalidation"
    );

    invalidate_skill_scan_cache();

    let refreshed = get_managed_skills_overview()
        .await
        .expect("refreshed overview");
    assert!(
        refreshed
            .user_skills
            .iter()
            .all(|skill| skill.name != skill_name),
        "overview should refresh after invalidation"
    );
}
