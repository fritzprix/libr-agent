use crate::common;

use std::path::PathBuf;
use tauri_mcp_agent_lib::repositories::{AssistantRepository, SqliteAssistantRepository};
use tauri_mcp_agent_lib::services::assistant_init::{
    ensure_default_assistants, load_bundled_assistants,
};

fn manifest_resource_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[tokio::test]
async fn test_load_bundled_assistants_from_manifest() {
    let assistants = load_bundled_assistants(&manifest_resource_dir())
        .expect("bundled assistants should load from manifest directory");

    assert_eq!(
        assistants.len(),
        4,
        "expected exactly four bundled assistants"
    );

    let names: Vec<&str> = assistants.iter().map(|a| a.name.as_str()).collect();
    assert!(names.contains(&"Libr Assistant"));
    assert!(names.contains(&"Coding Expert"));
    assert!(names.contains(&"App Wizard"));
    assert!(names.contains(&"Master Mind"));
    assert!(
        !names.contains(&"master-mind"),
        "legacy master-mind directory must not be loaded"
    );

    // Deep parity verification for Master Mind
    let master_mind = assistants
        .iter()
        .find(|a| a.name == "Master Mind")
        .expect("Master Mind should be loaded");

    let expected_prompt = std::fs::read_to_string(
        manifest_resource_dir().join("bundled_assistants/Master Mind/prompt.md"),
    )
    .expect("failed to read expected prompt");
    assert_eq!(master_mind.prompt, expected_prompt);

    assert_eq!(
        master_mind.config.allowed_builtin_service_aliases,
        vec![
            "planning".to_string(),
            "attachments".to_string(),
            "playbook".to_string(),
            "agent".to_string(),
        ]
    );

    // Deep parity verification for Libr Assistant (including checking its sample skill)
    let libr_assistant = assistants
        .iter()
        .find(|a| a.name == "Libr Assistant")
        .expect("Libr Assistant should be loaded");

    let expected_libr_prompt = std::fs::read_to_string(
        manifest_resource_dir().join("bundled_assistants/Libr Assistant/prompt.md"),
    )
    .expect("failed to read expected prompt");
    assert_eq!(libr_assistant.prompt, expected_libr_prompt);

    assert_eq!(
        libr_assistant.config.allowed_builtin_service_aliases,
        vec![
            "attachments".to_string(),
            "workspace".to_string(),
            "browser".to_string(),
            "planning".to_string(),
            "playbook".to_string(),
        ]
    );
}

#[tokio::test]
async fn test_ensure_default_assistants_hardcoded_fallback() {
    let db = common::setup_test_db_with_migrations().await;
    common::register_assistant_repository(&db);

    ensure_default_assistants(None)
        .await
        .expect("hardcoded fallback path should succeed");

    let repo = SqliteAssistantRepository::new(db);
    let assistants = repo
        .list_assistants()
        .await
        .expect("failed to list assistants");
    assert_eq!(assistants.len(), 4);

    let names: Vec<String> = assistants.into_iter().map(|a| a.name).collect();
    assert!(names.contains(&"Libr Assistant".to_string()));
    assert!(names.contains(&"Coding Expert".to_string()));
    assert!(names.contains(&"App Wizard".to_string()));
    assert!(names.contains(&"Master Mind".to_string()));
}

#[tokio::test]
async fn test_ensure_default_assistants_from_bundle() {
    let db = common::setup_test_db_with_migrations().await;
    common::register_assistant_repository(&db);

    ensure_default_assistants(Some(&manifest_resource_dir()))
        .await
        .expect("bundle path should seed assistants");

    let repo = SqliteAssistantRepository::new(db);
    let assistants = repo
        .list_assistants()
        .await
        .expect("failed to list assistants");
    assert_eq!(assistants.len(), 4);

    let names: Vec<String> = assistants.into_iter().map(|a| a.name).collect();
    assert!(names.contains(&"Libr Assistant".to_string()));
    assert!(names.contains(&"Coding Expert".to_string()));
    assert!(names.contains(&"App Wizard".to_string()));
    assert!(names.contains(&"Master Mind".to_string()));
}

#[tokio::test]
async fn test_zombie_assistant_cleanup_bundle_path() {
    let db = common::setup_test_db_with_migrations().await;
    common::register_assistant_repository(&db);

    let repo = SqliteAssistantRepository::new(db.clone());

    // 1. Seed a zombie default assistant with deletionProtected = true
    let zombie_id = "zombie-id-123".to_string();
    let zombie_name = "Legacy Zombie Assistant".to_string();
    let zombie_config = serde_json::json!({
        "description": "Legacy assistant to be deleted",
        "deletionProtected": true,
        "allowedBuiltInServiceAliases": []
    });
    repo.create_assistant(
        zombie_id.clone(),
        zombie_name.clone(),
        zombie_config.to_string(),
    )
    .await
    .expect("failed to seed zombie assistant");

    // 2. Seed a user assistant with deletionProtected = false
    let user_id = "user-id-456".to_string();
    let user_name = "User Custom Assistant".to_string();
    let user_config = serde_json::json!({
        "description": "User assistant to keep",
        "deletionProtected": false,
        "allowedBuiltInServiceAliases": []
    });
    repo.create_assistant(user_id.clone(), user_name.clone(), user_config.to_string())
        .await
        .expect("failed to seed user assistant");

    // 3. Run ensure_default_assistants (from bundle)
    ensure_default_assistants(Some(&manifest_resource_dir()))
        .await
        .expect("reconcile should succeed");

    // 4. Assert zombie is deleted, but user assistant and bundle assistants exist
    let assistants = repo.list_assistants().await.expect("failed to list");
    let names: Vec<String> = assistants.iter().map(|a| a.name.clone()).collect();

    assert!(
        !names.contains(&zombie_name),
        "zombie assistant must be deleted"
    );
    assert!(
        names.contains(&user_name),
        "user custom assistant must be kept"
    );
    assert!(
        names.contains(&"Master Mind".to_string()),
        "Master Mind must exist"
    );
}

#[tokio::test]
async fn test_zombie_assistant_cleanup_fallback_path() {
    let db = common::setup_test_db_with_migrations().await;
    common::register_assistant_repository(&db);

    let repo = SqliteAssistantRepository::new(db.clone());

    // 1. Seed a zombie default assistant with deletionProtected = true
    let zombie_id = "zombie-id-fallback".to_string();
    let zombie_name = "Legacy Zombie Assistant Fallback".to_string();
    let zombie_config = serde_json::json!({
        "description": "Legacy assistant to be deleted",
        "deletionProtected": true,
        "allowedBuiltInServiceAliases": []
    });
    repo.create_assistant(
        zombie_id.clone(),
        zombie_name.clone(),
        zombie_config.to_string(),
    )
    .await
    .expect("failed to seed zombie assistant");

    // 2. Seed a user assistant with deletionProtected = false
    let user_id = "user-id-fallback".to_string();
    let user_name = "User Custom Assistant Fallback".to_string();
    let user_config = serde_json::json!({
        "description": "User assistant to keep",
        "deletionProtected": false,
        "allowedBuiltInServiceAliases": []
    });
    repo.create_assistant(user_id.clone(), user_name.clone(), user_config.to_string())
        .await
        .expect("failed to seed user assistant");

    // 3. Run ensure_default_assistants (fallback path)
    ensure_default_assistants(None)
        .await
        .expect("reconcile should succeed");

    // 4. Assert zombie is deleted, but user assistant and bundle assistants exist
    let assistants = repo.list_assistants().await.expect("failed to list");
    let names: Vec<String> = assistants.iter().map(|a| a.name.clone()).collect();

    assert!(
        !names.contains(&zombie_name),
        "zombie assistant must be deleted"
    );
    assert!(
        names.contains(&user_name),
        "user custom assistant must be kept"
    );
    assert!(
        names.contains(&"Master Mind".to_string()),
        "Master Mind must exist"
    );
}

#[tokio::test]
async fn test_sync_assistant_bundled_skills() {
    let db = common::setup_test_db_with_migrations().await;
    common::register_assistant_repository(&db);

    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let target_dir = temp_dir.path().to_path_buf();

    // Call sync_assistant_bundled_skills
    tauri_mcp_agent_lib::lifecycle::app_setup::sync_assistant_bundled_skills(
        &manifest_resource_dir(),
        &target_dir,
    )
    .await
    .expect("skills sync should succeed");

    // Assert that the sample_helper skill was copied to the target directory under the Libr Assistant's ID
    let repo = SqliteAssistantRepository::new(db);
    let assistants = repo.list_assistants().await.expect("failed to list");
    let libr = assistants
        .iter()
        .find(|a| a.name == "Libr Assistant")
        .expect("Libr Assistant should exist");

    let source_skill_path = manifest_resource_dir()
        .join("bundled_assistants/Libr Assistant/bundled_skills/sample_helper/SKILL.md");

    let target_skill_path = target_dir
        .join("assistants")
        .join(&libr.id)
        .join("skills/sample_helper/SKILL.md");

    assert!(
        target_skill_path.exists(),
        "sample_helper/SKILL.md should be synced to disk"
    );

    let source_content =
        std::fs::read_to_string(source_skill_path).expect("failed to read source skill");
    let target_content =
        std::fs::read_to_string(target_skill_path).expect("failed to read synced skill");
    assert_eq!(
        source_content, target_content,
        "synced skill content must exactly match source content"
    );
}
