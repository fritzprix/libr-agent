use crate::mcp::types::MCPServerConfig;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::state::get_mcp_server_repository;

pub(super) async fn save_server_config(config: &MCPServerConfig) -> Result<String, String> {
    let repo = get_mcp_server_repository();
    let server_name = config
        .name
        .as_ref()
        .ok_or_else(|| "Server name is required".to_string())?;

    let config_value = serde_json::to_value(config).map_err(|e| e.to_string())?;

    // Save-first (pending). Connectivity runs asynchronously so registration is not blocked
    // by cold npx/uvx downloads. Callers can use tool__verifyServer for an explicit check.
    let (id, needs_probe) = match repo.get_by_name(server_name).await {
        Ok(Some(existing)) => {
            let existing_config_val: serde_json::Value = serde_json::from_str(&existing.config)
                .map_err(|e| format!("Failed to parse existing config from DB: {}", e))?;
            let requires_reverification = existing_config_val.get("transport")
                != config_value.get("transport")
                || existing_config_val.get("authentication") != config_value.get("authentication");

            let updated = repo
                .update(&existing.id, None, Some(config_value))
                .await
                .map_err(|e| format!("Failed to update MCP server config: {}", e))?;

            if requires_reverification {
                repo.mark_verification_pending(&updated.id, true)
                    .await
                    .map_err(|e| format!("Failed to mark verification pending: {}", e))?;
            }

            (updated.id, requires_reverification)
        }
        Ok(None) => {
            let created = repo
                .create(server_name, config_value)
                .await
                .map_err(|e| format!("Failed to create MCP server config: {}", e))?;
            (created.id, true)
        }
        Err(e) => return Err(format!("DB query error while saving server config: {}", e)),
    };

    if needs_probe {
        crate::services::McpServerService::schedule_background_probe(id.clone());
    }

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
