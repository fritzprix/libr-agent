use std::fs;

use tauri_mcp_agent_lib::services::skill_service::{get_skill_content_from_roots, SKILL_FILE_NAME};

fn create_skill_file(root: &tempfile::TempDir, skill_dir_name: &str, content: &str) -> String {
    let skill_dir = root.path().join(skill_dir_name);
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    let skill_file = skill_dir.join(SKILL_FILE_NAME);
    fs::write(&skill_file, content).expect("write skill file");
    skill_file.to_string_lossy().to_string()
}

#[tokio::test]
async fn get_skill_content_rejects_non_skill_markdown_paths() {
    let root = tempfile::tempdir().expect("tempdir");
    let bad_file = root.path().join("secret.txt");
    fs::write(&bad_file, "secret").expect("write non-skill file");

    let result = get_skill_content_from_roots(
        bad_file.to_string_lossy().to_string(),
        &[root.path().to_path_buf()],
    )
    .await;

    assert_eq!(
        result.unwrap_err(),
        "Skill path must point to a SKILL.md file"
    );
}

#[tokio::test]
async fn get_skill_content_blocks_paths_outside_allowed_roots() {
    let allowed_root = tempfile::tempdir().expect("allowed root");
    let outside_root = tempfile::tempdir().expect("outside root");
    let skill_path = create_skill_file(
        &outside_root,
        "outside-skill",
        "---\nname: outside\ndescription: blocked\n---\n# Outside\n",
    );

    let result =
        get_skill_content_from_roots(skill_path, &[allowed_root.path().to_path_buf()]).await;

    assert_eq!(
        result.unwrap_err(),
        "Skill path is outside the allowed skills directories"
    );
}

#[tokio::test]
async fn get_skill_content_reads_skill_inside_allowed_root() {
    let allowed_root = tempfile::tempdir().expect("allowed root");
    let expected_content = "---\nname: my-skill\ndescription: Does cool things.\n---\n# My Skill\n";
    let skill_path = create_skill_file(&allowed_root, "my-skill", expected_content);

    let result =
        get_skill_content_from_roots(skill_path, &[allowed_root.path().to_path_buf()]).await;

    assert_eq!(result.expect("skill content"), expected_content);
}
