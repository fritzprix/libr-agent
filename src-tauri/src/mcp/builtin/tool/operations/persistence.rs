use crate::mcp::types::MCPServerConfig;
use crate::mcp::utils::sanitize_mcp_server_name;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::services::McpServerService;
use crate::state::get_mcp_server_repository;

pub(super) async fn save_server_config(config: &MCPServerConfig) -> Result<String, String> {
    let repo = get_mcp_server_repository();
    let raw_name = config
        .name
        .as_ref()
        .ok_or_else(|| "Server name is required".to_string())?;
    let sanitized_name = sanitize_mcp_server_name(raw_name);

    // Save-first (pending). Connectivity runs asynchronously so registration is not blocked
    // by cold npx/uvx downloads. Callers can use tool__verifyServer for an explicit check.
    // Prefer sanitized name; fall back to raw for legacy rows that still contain spaces.
    let existing = match repo.get_by_name(&sanitized_name).await {
        Ok(Some(model)) => Some(model),
        Ok(None) if sanitized_name != *raw_name => repo
            .get_by_name(raw_name)
            .await
            .map_err(|e| format!("DB query error while saving server config: {}", e))?,
        Ok(None) => None,
        Err(e) => return Err(format!("DB query error while saving server config: {}", e)),
    };

    let (id, needs_probe) = match existing {
        Some(existing) => {
            // Keep legacy display names (including whitespace) intact.
            let mut config = config.clone();
            config.name = Some(existing.name.clone());
            let config_value = serde_json::to_value(&config).map_err(|e| e.to_string())?;

            let existing_config_val: serde_json::Value = serde_json::from_str(&existing.config)
                .map_err(|e| format!("Failed to parse existing config from DB: {}", e))?;
            let requires_reverification = existing_config_val.get("transport")
                != config_value.get("transport")
                || existing_config_val.get("authentication") != config_value.get("authentication");

            let updated = McpServerService::update_server_config(
                repo,
                existing.id.clone(),
                None,
                Some(config_value),
            )
            .await?;

            (updated.id, requires_reverification)
        }
        None => {
            let mut config = config.clone();
            config.name = Some(sanitized_name.clone());
            let config_value = serde_json::to_value(&config).map_err(|e| e.to_string())?;
            let created =
                McpServerService::create_server_config(repo, sanitized_name, config_value).await?;
            (created.id, true)
        }
    };

    if needs_probe {
        McpServerService::schedule_background_probe(id.clone());
    }

    Ok(id)
}

pub(super) async fn delete_server_config_db(id_or_name: String) -> Result<(), String> {
    let repo = get_mcp_server_repository();

    // Try ID first, then name (raw and sanitized for legacy whitespace rows)
    let mut server = repo.get(&id_or_name).await.map_err(|e| e.to_string())?;
    if server.is_none() {
        server = repo
            .get_by_name(&id_or_name)
            .await
            .map_err(|e| e.to_string())?;
    }
    if server.is_none() {
        let sanitized = sanitize_mcp_server_name(&id_or_name);
        if sanitized != id_or_name {
            server = repo
                .get_by_name(&sanitized)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    let server = server.ok_or_else(|| format!("MCP server '{}' not found", id_or_name))?;

    McpServerService::delete_server_config(repo, &server.id).await
}
