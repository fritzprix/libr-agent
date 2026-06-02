/// Integration tests for SqliteMCPServerRepository.
///
/// These tests replace #[cfg(test)] unit tests that cannot run via `cargo test --lib`
/// on Windows (STATUS_ENTRYPOINT_NOT_FOUND DLL issue). CI uses `cargo test --tests`.
use crate::common;

use tauri_mcp_agent_lib::repositories::{MCPServerRepository, SqliteMCPServerRepository};

async fn setup_repo() -> SqliteMCPServerRepository {
    let db = common::setup_test_db_with_migrations().await;
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
    assert_eq!(retrieved.verification_status.as_deref(), Some("pending"));
    assert!(retrieved.last_verification_error.is_none());

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
    assert_eq!(result.verification_status.as_deref(), Some("success"));
    assert!(result.last_verification_error.is_none());
}

#[tokio::test]
async fn test_mark_verification_pending_clears_cache_when_requested() {
    let repo = setup_repo().await;
    let created = repo
        .create("pending_server", serde_json::json!({"v": 1}))
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
    assert_eq!(cached.verification_status.as_deref(), Some("success"));

    repo.mark_verification_pending(&created.id, true)
        .await
        .expect("Failed to mark verification pending");

    let after = repo.get(&created.id).await.unwrap().unwrap();
    assert_eq!(after.verification_status.as_deref(), Some("pending"));
    assert!(after.last_verification_error.is_none());
    assert!(
        after.tool_count.is_none(),
        "tool_count must be cleared when verification is pending with clear_cache=true"
    );
    assert!(
        after.cached_tools.is_none(),
        "cached_tools must be cleared when verification is pending with clear_cache=true"
    );
}

#[tokio::test]
async fn test_update_preserves_tool_cache_until_service_invalidates_it() {
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

    repo.update(
        &created.id,
        Some("after_rename"),
        Some(serde_json::json!({"v": 2})),
    )
    .await
    .expect("Failed to update");

    let after = repo.get(&created.id).await.unwrap().unwrap();
    assert_eq!(
        after.tool_count,
        Some(3),
        "repository update should preserve cache until service decides whether re-verification is needed"
    );
    assert!(
        after.cached_tools.is_some(),
        "repository update should preserve cached_tools until service decides whether re-verification is needed"
    );
}

#[tokio::test]
async fn test_set_verification_error_persists_error_message() {
    let repo = setup_repo().await;
    let created = repo
        .create("error_server", serde_json::json!({}))
        .await
        .unwrap();

    repo.set_verification_error(&created.id, "boom".to_string())
        .await
        .expect("Failed to persist verification error");

    let after = repo.get(&created.id).await.unwrap().unwrap();
    assert_eq!(after.verification_status.as_deref(), Some("error"));
    assert_eq!(after.last_verification_error.as_deref(), Some("boom"));
}
