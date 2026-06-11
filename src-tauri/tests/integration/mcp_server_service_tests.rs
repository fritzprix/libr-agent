use crate::common;

use tauri_mcp_agent_lib::repositories::{MCPServerRepository, SqliteMCPServerRepository};
use tauri_mcp_agent_lib::services::McpServerService;

async fn setup_repo() -> SqliteMCPServerRepository {
    let db = common::setup_test_db_with_migrations().await;
    SqliteMCPServerRepository::new(db)
}

fn missing_stdio_wrapper_config() -> serde_json::Value {
    serde_json::json!({
        "transport": {
            "type": "stdio",
            "command": "/tmp/libragent-missing-wrapper/npx.sh",
            "args": ["-y", "@example/mcp-server"]
        }
    })
}

#[tokio::test]
async fn test_create_server_config_rejects_unreachable_stdio_before_save() {
    let repo = setup_repo().await;

    let result = McpServerService::create_server_config(
        &repo,
        "bad-server".to_string(),
        missing_stdio_wrapper_config(),
    )
    .await;

    let error = result.expect_err("expected create to fail verification");
    assert!(
        error.contains("Verification failed"),
        "unexpected error message: {error}"
    );

    assert!(
        repo.list().await.unwrap().is_empty(),
        "invalid config must not be persisted"
    );
}

#[tokio::test]
async fn test_update_server_config_rejects_unreachable_stdio_before_save() {
    let repo = setup_repo().await;
    let created = repo
        .create(
            "yahoo-finance",
            serde_json::json!({
                "transport": {
                    "type": "stdio",
                    "command": "npx",
                    "args": ["-y", "@fre4x/yahoo-finance"]
                }
            }),
        )
        .await
        .unwrap();

    let result = McpServerService::update_server_config(
        &repo,
        created.id.clone(),
        None,
        Some(missing_stdio_wrapper_config()),
    )
    .await;

    let error = result.expect_err("expected update to fail verification");
    assert!(
        error.contains("Verification failed"),
        "unexpected error message: {error}"
    );

    let persisted = repo.get(&created.id).await.unwrap().unwrap();
    assert_eq!(
        persisted.config, created.config,
        "failed transport update must not overwrite existing config"
    );
}
