/// Integration tests for SqliteMCPServerRepository.
///
/// These tests replace #[cfg(test)] unit tests that cannot run via `cargo test --lib`
/// on Windows (STATUS_ENTRYPOINT_NOT_FOUND DLL issue). CI uses `cargo test --tests`.
use sea_orm::Database;
use sea_orm_migration::MigratorTrait;
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::repositories::{MCPServerRepository, SqliteMCPServerRepository};

async fn setup_repo() -> SqliteMCPServerRepository {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");
    Migrator::up(&db, None)
        .await
        .expect("Migrations should run");
    SqliteMCPServerRepository::new(db)
}

#[tokio::test]
async fn test_create_and_get_server() {
    let repo = setup_repo().await;
    let config = serde_json::json!({"cmd": "test"});

    let saved = repo
        .create("test_server", config.clone())
        .await
        .expect("Failed to create server");
    assert_eq!(saved.name, "test_server");
    assert_eq!(saved.config, config.to_string());
    assert!(!saved.id.is_empty());

    let retrieved = repo.get(&saved.id).await.unwrap().unwrap();
    assert_eq!(retrieved.name, "test_server");
    assert_eq!(retrieved.id, saved.id);

    let by_name = repo.get_by_name("test_server").await.unwrap().unwrap();
    assert_eq!(by_name.id, saved.id);
}

#[tokio::test]
async fn test_update_server() {
    let repo = setup_repo().await;
    let created = repo
        .create("update_server", serde_json::json!({"v": 1}))
        .await
        .unwrap();

    // Config-only update
    let updated = repo
        .update(&created.id, None, Some(serde_json::json!({"v": 2})))
        .await
        .expect("Failed to update config");
    assert_eq!(updated.config, r#"{"v":2}"#);
    assert_eq!(updated.name, "update_server");

    // Name-only update
    let renamed = repo
        .update(&created.id, Some("renamed_server"), None)
        .await
        .expect("Failed to rename");
    assert_eq!(renamed.name, "renamed_server");
    let by_new_name = repo.get_by_name("renamed_server").await.unwrap().unwrap();
    assert_eq!(by_new_name.id, created.id);
}

#[tokio::test]
async fn test_delete_server() {
    let repo = setup_repo().await;
    let created = repo
        .create("delete_server", serde_json::json!({}))
        .await
        .unwrap();
    repo.delete(&created.id).await.expect("Failed to delete");
    assert!(repo.get(&created.id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_update_cached_tools() {
    let repo = setup_repo().await;
    let created = repo
        .create("cache_server", serde_json::json!({}))
        .await
        .unwrap();

    let tools_json = r#"[{"name":"foo","description":"does foo"}]"#.to_string();
    repo.update_cached_tools(&created.id, 1, tools_json.clone())
        .await
        .expect("Failed to update cached tools");

    let result = repo.get(&created.id).await.unwrap().unwrap();
    assert_eq!(result.tool_count, Some(1));
    assert_eq!(result.cached_tools.as_deref(), Some(tools_json.as_str()));
}

#[tokio::test]
async fn test_config_update_invalidates_tool_cache() {
    // Regression: updating config must clear BOTH cached_tools AND tool_count.
    let repo = setup_repo().await;
    let created = repo
        .create("invalidate_server", serde_json::json!({"v": 1}))
        .await
        .unwrap();

    repo.update_cached_tools(
        &created.id,
        5,
        r#"[{"name":"t1","description":""}]"#.to_string(),
    )
    .await
    .unwrap();

    // Verify cache is populated
    let cached = repo.get(&created.id).await.unwrap().unwrap();
    assert_eq!(cached.tool_count, Some(5));
    assert!(cached.cached_tools.is_some());

    // Update config — must invalidate both
    repo.update(&created.id, None, Some(serde_json::json!({"v": 2})))
        .await
        .expect("Failed to update config");

    let after = repo.get(&created.id).await.unwrap().unwrap();
    assert!(
        after.tool_count.is_none(),
        "tool_count must be cleared when config changes"
    );
    assert!(
        after.cached_tools.is_none(),
        "cached_tools must be cleared when config changes"
    );
}

#[tokio::test]
async fn test_name_only_update_preserves_tool_cache() {
    // Regression: renaming a server must NOT invalidate the tool cache.
    let repo = setup_repo().await;
    let created = repo
        .create("before_rename", serde_json::json!({}))
        .await
        .unwrap();

    repo.update_cached_tools(
        &created.id,
        3,
        r#"[{"name":"t1","description":""}]"#.to_string(),
    )
    .await
    .unwrap();

    // Rename only — no config change
    repo.update(&created.id, Some("after_rename"), None)
        .await
        .expect("Failed to rename");

    let after = repo.get(&created.id).await.unwrap().unwrap();
    assert_eq!(
        after.tool_count,
        Some(3),
        "tool_count must be preserved on name-only update"
    );
    assert!(
        after.cached_tools.is_some(),
        "cached_tools must be preserved on name-only update"
    );
}
