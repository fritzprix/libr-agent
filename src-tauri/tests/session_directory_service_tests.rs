use tauri_mcp_agent_lib::services::skill_service::{
    LEGACY_SYSTEM_SKILLS_DIR_NAME, SYSTEM_SKILLS_DIR_NAME, USER_SKILLS_DIR_NAME,
};
use tauri_mcp_agent_lib::services::SessionDirectoryService;
use tempfile::TempDir;

#[test]
fn session_directory_service_skips_legacy_skills_directory() {
    let base_data_dir = TempDir::new().unwrap();
    let service = SessionDirectoryService::new(base_data_dir.path().to_path_buf()).unwrap();

    assert!(service
        .get_base_data_dir()
        .join(SYSTEM_SKILLS_DIR_NAME)
        .exists());
    assert!(service
        .get_base_data_dir()
        .join(USER_SKILLS_DIR_NAME)
        .exists());
    assert!(!service
        .get_base_data_dir()
        .join(LEGACY_SYSTEM_SKILLS_DIR_NAME)
        .exists());
}
