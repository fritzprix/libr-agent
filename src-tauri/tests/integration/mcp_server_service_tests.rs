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
async fn test_create_server_config_saves_pending_without_blocking_on_verify() {
    let repo = setup_repo().await;

    let model = McpServerService::create_server_config(
        &repo,
        "bad-server".to_string(),
        missing_stdio_wrapper_config(),
    )
    .await
    .expect("create must succeed immediately (save-first)");

    assert_eq!(model.name, "bad-server");
    assert_eq!(
        model.verification_status.as_deref(),
        Some("pending"),
        "new installs start as pending until background probe finishes"
    );

    let listed = repo.list().await.unwrap();
    assert_eq!(
        listed.len(),
        1,
        "unreachable config must still be persisted"
    );

    // Explicit probe marks the saved row as error instead of rejecting create.
    let probe_err = McpServerService::probe_server(&repo, &model.id)
        .await
        .expect_err("unreachable stdio should fail probe");
    assert!(
        probe_err.contains("Failed to connect") || probe_err.contains("Timed out"),
        "unexpected probe error: {probe_err}"
    );

    let after = repo.get(&model.id).await.unwrap().unwrap();
    assert_eq!(after.verification_status.as_deref(), Some("error"));
    assert!(after.last_verification_error.is_some());
}

#[tokio::test]
async fn test_update_server_config_marks_pending_for_unreachable_transport_change() {
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

    let updated = McpServerService::update_server_config(
        &repo,
        created.id.clone(),
        None,
        Some(missing_stdio_wrapper_config()),
    )
    .await
    .expect("transport update must persist even when target is unreachable");

    assert_eq!(
        updated.verification_status.as_deref(),
        Some("pending"),
        "transport changes should clear cache and mark pending"
    );

    let persisted = repo.get(&created.id).await.unwrap().unwrap();
    let config: serde_json::Value = serde_json::from_str(&persisted.config).unwrap();
    assert_eq!(
        config["transport"]["command"], "/tmp/libragent-missing-wrapper/npx.sh",
        "updated transport must be written before background probe"
    );
}

#[tokio::test]
async fn test_create_rejects_reserved_builtin_name() {
    let repo = setup_repo().await;

    let result = McpServerService::create_server_config(
        &repo,
        "workspace".to_string(),
        missing_stdio_wrapper_config(),
    )
    .await;

    let error = result.expect_err("reserved names must still be rejected");
    assert!(
        error.contains("reserved"),
        "unexpected error message: {error}"
    );
    assert!(repo.list().await.unwrap().is_empty());
}
