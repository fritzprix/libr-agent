use crate::mcp::types::MCPServerConfig;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::state::get_mcp_server_repository;

pub(super) async fn save_server_config(config: &MCPServerConfig) -> Result<String, String> {
    let repo = get_mcp_server_repository();
    let server_name = config
        .name
        .as_ref()
        .ok_or_else(|| "Server name is required".to_string())?;

    // 1. Verify the configuration before saving
    let tools =
        crate::services::mcp_server_service::McpServerService::verify_config(config.clone())
            .await
            .map_err(|e| format!("Verification failed: {}", e))?;
    let tools_json_str = crate::mcp::utils::serialize_mcp_tools(&tools);

    let config_value = serde_json::to_value(config).map_err(|e| e.to_string())?;

    // Try to update first (by name lookup), create if doesn't exist
    let id = match repo.get_by_name(server_name).await {
        Ok(Some(existing)) => {
            // Update by ID with new config
            repo.update(&existing.id, None, Some(config_value))
                .await
                .map_err(|e| format!("Failed to update MCP server config: {}", e))?
                .id
        }
        Ok(None) => {
            repo.create(server_name, config_value)
                .await
                .map_err(|e| format!("Failed to create MCP server config: {}", e))?
                .id
        }
        Err(e) => return Err(format!("DB query error while saving server config: {}", e)),
    };

    // Update the cached tools immediately since we just verified it
    let _ = repo
        .update_cached_tools(&id, tools.len() as i32, tools_json_str)
        .await;

    Ok(id)
}

pub(super) async fn delete_server_config_db(id_or_name: String) -> Result<(), String> {
    let repo = get_mcp_server_repository();

    // Try ID first, then name
    let mut server = repo.get(&id_or_name).await.map_err(|e| e.to_string())?;
    if server.is_none() {
        server = repo
            .get_by_name(&id_or_name)
            .await
            .map_err(|e| e.to_string())?;
    }

    let server = server.ok_or_else(|| format!("MCP server '{}' not found", id_or_name))?;

    repo.delete(&server.id)
        .await
        .map_err(|e| format!("DB Delete Error: {}", e))?;
    Ok(())
}
