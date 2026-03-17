use crate::mcp::types::MCPServerConfig;
use crate::repositories::mcp_server_repository::MCPServerRepository;
use crate::state::get_mcp_server_repository;

/// Get server ID and configuration by ID or name
pub async fn get_server_details(
    id_or_name: &str,
) -> Result<Option<(String, MCPServerConfig)>, String> {
    let repo = get_mcp_server_repository();

    // Try ID first
    let mut model = repo
        .get(id_or_name)
        .await
        .map_err(|e| format!("DB Fetch Error: {}", e))?;

    // Fallback to name
    if model.is_none() {
        model = repo
            .get_by_name(id_or_name)
            .await
            .map_err(|e| format!("DB Fetch Error: {}", e))?;
    }

    if let Some(model) = model {
        let mut config: MCPServerConfig =
            serde_json::from_str(&model.config).map_err(|e| e.to_string())?;
        config.name = Some(model.name.clone());
        Ok(Some((model.id, config)))
    } else {
        Ok(None)
    }
}

/// Get a server configuration by name or ID
pub async fn get_server_config(id_or_name: &str) -> Result<Option<MCPServerConfig>, String> {
    get_server_details(id_or_name)
        .await
        .map(|opt| opt.map(|(_, config)| config))
}
