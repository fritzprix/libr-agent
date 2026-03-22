use sea_orm::Database;
use sea_orm_migration::MigratorTrait;
use tauri_mcp_agent_lib::mcp::service_proxy_manager::persist_tool_cache_for_server;
use tauri_mcp_agent_lib::mcp::types::MCPTool;
use tauri_mcp_agent_lib::migration::Migrator;
use tauri_mcp_agent_lib::repositories::{MCPServerRepository, SqliteMCPServerRepository};
use tauri_mcp_agent_lib::set_mcp_server_repository;

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
async fn persist_tool_cache_refreshes_cached_tools_and_tool_count() {
    let repo = setup_repo().await;
    set_mcp_server_repository(repo.clone());

    let created = repo
        .create("gemini", serde_json::json!({"command": "npx"}))
        .await
        .expect("Failed to create server");

    repo.update_cached_tools(
        &created.id,
        5,
        r#"[{"name":"generate_text","description":"old"},{"name":"analyze_media","description":"old"}]"#
            .to_string(),
    )
    .await
    .expect("Failed to seed stale cache");

    let tools = vec![
        MCPTool {
            name: "analyze_media".to_string(),
            title: None,
            description: "Analyze media".to_string(),
            input_schema: Default::default(),
            output_schema: None,
            annotations: None,
        },
        MCPTool {
            name: "list_models".to_string(),
            title: None,
            description: "List models".to_string(),
            input_schema: Default::default(),
            output_schema: None,
            annotations: None,
        },
        MCPTool {
            name: "generate_image".to_string(),
            title: None,
            description: "Generate image".to_string(),
            input_schema: Default::default(),
            output_schema: None,
            annotations: None,
        },
        MCPTool {
            name: "generate_video".to_string(),
            title: None,
            description: "Generate video".to_string(),
            input_schema: Default::default(),
            output_schema: None,
            annotations: None,
        },
        MCPTool {
            name: "get_video_status".to_string(),
            title: None,
            description: "Get video status".to_string(),
            input_schema: Default::default(),
            output_schema: None,
            annotations: None,
        },
    ];

    persist_tool_cache_for_server("gemini", Some(created.id.as_str()), "stdio", &tools).await;

    let updated = repo
        .get(&created.id)
        .await
        .expect("Failed to reload server")
        .expect("Server should still exist");

    assert_eq!(updated.tool_count, Some(5));

    let cached_tools = updated
        .cached_tools
        .expect("cached_tools should be refreshed after discovery");
    assert!(cached_tools.contains("get_video_status"));
    assert!(!cached_tools.contains("generate_text"));
}
